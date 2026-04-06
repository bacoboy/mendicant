/**
 * passkey-plugin.js
 *
 * Datastar plugin that adds @passkeyRegister() and @passkeyLogin() actions
 * for WebAuthn passkey-only authentication.
 *
 * Usage in HTML:
 *   <script type="module" src="...datastar.js"></script>
 *   <script type="module" src="/static/passkey-plugin.js"></script>
 *
 *   <!-- Signals that must exist on the page: -->
 *   <!--   register page: email, displayName, registerError -->
 *   <!--   login page:    loginError                        -->
 *
 *   <button data-on-click="@passkeyRegister($email, $displayName)">Register</button>
 *   <button data-on-click="@passkeyLogin()">Sign in</button>
 *
 * Keep the DATASTAR_URL constant in sync with the <script> tag in your HTML templates.
 */

// ── Binary helpers ────────────────────────────────────────────────────────────

/** base64url string → Uint8Array */
function b64urlToBytes(str) {
  const b64 = str.replace(/-/g, '+').replace(/_/g, '/');
  const padded = b64 + '='.repeat((4 - (b64.length % 4)) % 4);
  const bin = atob(padded);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** ArrayBuffer / Uint8Array → base64url string (no padding) */
function bytesToB64url(buf) {
  const bytes = buf instanceof Uint8Array ? buf : new Uint8Array(buf);
  // btoa has a 65536-byte argument limit in some environments; chunk if needed
  let b64 = '';
  const CHUNK = 0x8000;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    b64 += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  }
  return btoa(b64).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
}

// ── WebAuthn format converters ─────────────────────────────────────────────────

/**
 * Convert the server's CreationChallengeResponse (JSON, base64url binary)
 * to the format expected by navigator.credentials.create().
 * webauthn-rs wraps options in {publicKey: {...}}, which is exactly what
 * the browser API expects — we just decode the binary fields.
 */
function toCreationOptions(serverResponse) {
  const pk = { ...serverResponse.publicKey };
  pk.challenge = b64urlToBytes(pk.challenge);
  if (pk.user) {
    pk.user = { ...pk.user, id: b64urlToBytes(pk.user.id) };
  }
  if (pk.excludeCredentials?.length) {
    pk.excludeCredentials = pk.excludeCredentials.map((c) => ({
      ...c,
      id: b64urlToBytes(c.id),
    }));
  }
  // Filter extensions to only standard WebAuthn names (Safari compatibility)
  if (pk.extensions) {
    const standardExtensions = {};
    const standardNames = ['credProps', 'hmacGetSecret', 'hmacSecret', 'minPinLength'];
    for (const key of standardNames) {
      if (key in pk.extensions) {
        standardExtensions[key] = pk.extensions[key];
      }
    }
    if (Object.keys(standardExtensions).length > 0) {
      pk.extensions = standardExtensions;
    } else {
      delete pk.extensions;
    }
  }
  return { publicKey: pk };
}

/**
 * Convert the server's RequestChallengeResponse (JSON, base64url binary)
 * to the format expected by navigator.credentials.get().
 */
function toRequestOptions(serverResponse) {
  const pk = { ...serverResponse.publicKey };
  pk.challenge = b64urlToBytes(pk.challenge);
  if (pk.allowCredentials?.length) {
    pk.allowCredentials = pk.allowCredentials.map((c) => ({
      ...c,
      id: b64urlToBytes(c.id),
    }));
  }
  // Filter extensions to only standard WebAuthn names (Safari compatibility)
  if (pk.extensions) {
    const standardExtensions = {};
    const standardNames = ['credProps', 'hmacGetSecret', 'hmacSecret', 'minPinLength'];
    for (const key of standardNames) {
      if (key in pk.extensions) {
        standardExtensions[key] = pk.extensions[key];
      }
    }
    if (Object.keys(standardExtensions).length > 0) {
      pk.extensions = standardExtensions;
    } else {
      delete pk.extensions;
    }
  }
  return { publicKey: pk };
}

/**
 * Serialize a registration PublicKeyCredential to JSON for the server.
 * The server expects base64url-encoded binary fields (webauthn-rs convention).
 */
function serializeRegistration(cred) {
  return {
    id: cred.id,
    rawId: bytesToB64url(cred.rawId),
    type: cred.type,
    authenticatorAttachment: cred.authenticatorAttachment ?? null,
    clientExtensionResults: cred.getClientExtensionResults?.() ?? {},
    response: {
      attestationObject: bytesToB64url(cred.response.attestationObject),
      clientDataJSON: bytesToB64url(cred.response.clientDataJSON),
      ...(cred.response.getTransports
        ? { transports: cred.response.getTransports() }
        : {}),
    },
  };
}

/**
 * Serialize an authentication PublicKeyCredential to JSON for the server.
 */
function serializeAuthentication(cred) {
  return {
    id: cred.id,
    rawId: bytesToB64url(cred.rawId),
    type: cred.type,
    clientExtensionResults: cred.getClientExtensionResults?.() ?? {},
    response: {
      authenticatorData: bytesToB64url(cred.response.authenticatorData),
      clientDataJSON: bytesToB64url(cred.response.clientDataJSON),
      signature: bytesToB64url(cred.response.signature),
      userHandle: cred.response.userHandle
        ? bytesToB64url(cred.response.userHandle)
        : null,
    },
  };
}

// ── SSE helpers ───────────────────────────────────────────────────────────────

/**
 * POST to a Datastar SSE endpoint, parse the response stream, and return
 * a map of signals extracted from datastar-patch-signals events.
 * Also executes any datastar-execute-script events (e.g. redirects after login).
 */
async function fetchSSE(url, body) {
  const resp = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Accept: 'text/event-stream' },
    credentials: 'same-origin',
    body: JSON.stringify(body),
  });

  if (!resp.ok) {
    const text = await resp.text().catch(() => resp.statusText);
    throw new Error(`${resp.status}: ${text}`);
  }

  const signals = {};
  const reader = resp.body.getReader();
  const decoder = new TextDecoder();
  let buf = '';

  while (true) {
    const { value, done } = await reader.read();
    if (done) break;
    buf += decoder.decode(value, { stream: true });

    // SSE events are separated by blank lines (\n\n)
    const parts = buf.split('\n\n');
    buf = parts.pop(); // keep any incomplete trailing event

    for (const part of parts) {
      let eventType = '';
      const dataLines = [];
      for (const line of part.split('\n')) {
        if (line.startsWith('event:')) {
          eventType = line.slice(6).trim();
        } else if (line.startsWith('data:')) {
          dataLines.push(line.slice(5).trim());
        }
      }
      const data = dataLines.join('\n');

      if (eventType === 'datastar-patch-signals') {
        // Datastar prefix: "signals <json>"
        const json = data.startsWith('signals ') ? data.slice(8) : data;
        try {
          Object.assign(signals, JSON.parse(json));
        } catch (e) {
          console.warn('[passkey-plugin] Failed to parse patch-signals:', e, json);
        }
      } else if (eventType === 'datastar-execute-script') {
        // Datastar prefix: "script <code>"
        const code = data.startsWith('script ') ? data.slice(7) : data;
        try {
          // eslint-disable-next-line no-new-func
          new Function(code)();
        } catch (e) {
          console.warn('[passkey-plugin] Failed to execute script:', e, code);
        }
      }
    }
  }

  return signals;
}

// ── Signal helpers ─────────────────────────────────────────────────────────────

/**
 * Write a value to a Datastar signal.
 * Datastar v1.x exposes all signals on the global `root` reactive proxy.
 */
function setSignal(name, value) {
  if (window.root) {
    window.root[name] = value;
  }
}

// ── Action implementations ─────────────────────────────────────────────────────

/**
 * @registerEmail(email, displayName)
 *
 * Email validation step:
 *   1. POST /auth/register/email  → returns token
 *   2. Redirect to /register-confirm?token={token}
 */
async function doRegisterEmail(ctx, email) {
  setSignal('emailError', '');

  if (!email) {
    setSignal('emailError', 'Please enter your email address.');
    return;
  }

  try {
    const response = await fetch('/auth/register/email', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email }),
    });

    const data = await response.json();

    if (!response.ok) {
      setSignal('emailError', data.error || 'Failed to validate email. Please try again.');
      return;
    }

    // Success: redirect to confirm page with token
    window.location.href = `/register-confirm?token=${data.token}`;
  } catch (e) {
    setSignal('emailError', `Error: ${e.message}`);
  }
}

/**
 * @passkeyRegisterWithToken(token)
 *
 * Passkey registration with email token:
 *   1. POST /auth/register/begin  → challenge (uses token)
 *   2. navigator.credentials.create()
 *   3. POST /auth/register/complete → auth cookie + redirect
 */
async function doRegisterWithToken(ctx, token) {
  setSignal('registerError', '');

  if (!token) {
    setSignal('registerError', 'Invalid registration link. Please start over.');
    return;
  }

  const displayName = window.root?.displayName || window.Datastar?.root?.displayName;
  if (!displayName) {
    setSignal('registerError', 'Please enter your display name.');
    return;
  }

  // 1. Begin registration
  let beginSignals;
  try {
    beginSignals = await fetchSSE('/auth/register/begin', {
      token,
      display_name: displayName,
    });
  } catch (e) {
    setSignal('registerError', `Could not start registration: ${e.message}`);
    return;
  }

  const { challengeId, registerOptions } = beginSignals;
  if (!challengeId || !registerOptions) {
    setSignal('registerError', 'Server did not return registration options.');
    return;
  }

  // 2. Create credential in the authenticator
  let credential;
  try {
    credential = await navigator.credentials.create(toCreationOptions(registerOptions));
  } catch (e) {
    if (e.name === 'NotAllowedError') {
      setSignal('registerError', 'Passkey creation was cancelled.');
    } else {
      setSignal('registerError', `Passkey creation failed: ${e.message}`);
    }
    return;
  }

  if (!credential) {
    setSignal('registerError', 'No credential was returned by the authenticator.');
    return;
  }

  // 3. Complete registration (server sets HttpOnly auth cookie and redirects)
  try {
    await fetchSSE('/auth/register/complete', {
      challenge_id: challengeId,
      response: serializeRegistration(credential),
    });
  } catch (e) {
    setSignal('registerError', `Registration failed: ${e.message}`);
  }
}

/**
 * @passkeyRegister(email, displayName)
 *
 * Full passkey registration flow (legacy):
 *   1. POST /auth/register/begin  → challenge
 *   2. navigator.credentials.create()
 *   3. POST /auth/register/complete → auth cookie + redirect
 */
async function doRegister(ctx, email, displayName) {
  // Clear any previous error
  setSignal('registerError', '');

  if (!email) {
    setSignal('registerError', 'Please enter your email address.');
    return;
  }

  // 1. Begin registration
  let beginSignals;
  try {
    beginSignals = await fetchSSE('/auth/register/begin', {
      email,
      display_name: displayName || email.split('@')[0],
    });
  } catch (e) {
    setSignal('registerError', `Could not start registration: ${e.message}`);
    return;
  }

  const { challengeId, registerOptions } = beginSignals;
  if (!challengeId || !registerOptions) {
    setSignal('registerError', 'Server did not return registration options.');
    return;
  }

  // 2. Create credential in the authenticator
  let credential;
  try {
    credential = await navigator.credentials.create(toCreationOptions(registerOptions));
  } catch (e) {
    if (e.name === 'NotAllowedError') {
      setSignal('registerError', 'Passkey creation was cancelled.');
    } else {
      setSignal('registerError', `Passkey creation failed: ${e.message}`);
    }
    return;
  }

  if (!credential) {
    setSignal('registerError', 'No credential was returned by the authenticator.');
    return;
  }

  // 3. Complete registration (server sets HttpOnly auth cookie and redirects)
  try {
    await fetchSSE('/auth/register/complete', {
      challenge_id: challengeId,
      response: serializeRegistration(credential),
    });
  } catch (e) {
    setSignal('registerError', `Registration failed: ${e.message}`);
  }
}

/**
 * @passkeyLogin()
 *
 * Full passkey authentication flow (discovery mode — no email required):
 *   1. POST /auth/login/begin  → challenge (discovery mode)
 *   2. navigator.credentials.get()
 *   3. POST /auth/login/complete → auth cookie + redirect
 */
async function doLogin(ctx) {
  setSignal('loginError', '');

  // 1. Begin authentication (discovery mode: no email needed)
  let beginSignals;
  try {
    beginSignals = await fetchSSE('/auth/login/begin', {});
  } catch (e) {
    setSignal('loginError', `Could not start sign-in: ${e.message}`);
    return;
  }

  const { challengeId, loginOptions } = beginSignals;
  if (!challengeId || !loginOptions) {
    setSignal('loginError', 'Server did not return sign-in options.');
    return;
  }

  // 2. Sign the challenge with the authenticator
  let credential;
  try {
    credential = await navigator.credentials.get(toRequestOptions(loginOptions));
  } catch (e) {
    if (e.name === 'NotAllowedError') {
      setSignal('loginError', 'Sign-in was cancelled.');
    } else {
      setSignal('loginError', `Sign-in failed: ${e.message}`);
    }
    return;
  }

  if (!credential) {
    setSignal('loginError', 'No credential was returned by the authenticator.');
    return;
  }

  // 3. Complete authentication (server sets HttpOnly auth cookie and redirects)
  try {
    await fetchSSE('/auth/login/complete', {
      challenge_id: challengeId,
      response: serializeAuthentication(credential),
    });
  } catch (e) {
    setSignal('loginError', `Sign-in failed: ${e.message}`);
  }
}

// ── Event listener setup ──────────────────────────────────────────────────────

// Wait for DOM to be ready, then set up event listeners
document.addEventListener('DOMContentLoaded', () => {
  // Find email registration button (exact match)
  const emailRegisterBtn = document.querySelector('button[data-on-click="registerEmail"]');
  if (emailRegisterBtn) {
    emailRegisterBtn.addEventListener('click', async (e) => {
      e.preventDefault();
      const email = window.root?.email || window.Datastar?.root?.email;
      console.log('[passkey-plugin] Email register clicked:', { email });
      await doRegisterEmail(null, email);
    });
    console.log('[passkey-plugin] Email register button listener attached');
  }

  // Find token-based passkey registration button (exact match)
  const tokenRegisterBtn = document.querySelector('button[data-on-click="passkeyRegisterWithToken"]');
  if (tokenRegisterBtn) {
    tokenRegisterBtn.addEventListener('click', async (e) => {
      e.preventDefault();
      const token = window.root?.token || window.Datastar?.root?.token;
      console.log('[passkey-plugin] Token register clicked:', { token });
      await doRegisterWithToken(null, token);
    });
    console.log('[passkey-plugin] Token register button listener attached');
  }

  // Find register button (substring match for @passkeyRegister, but not passkeyRegisterWithToken)
  const registerBtn = document.querySelector('button[data-on-click*="passkeyRegister"]:not([data-on-click*="WithToken"])');
  if (registerBtn) {
    registerBtn.addEventListener('click', async (e) => {
      e.preventDefault();
      const email = window.root?.email || window.Datastar?.root?.email;
      const displayName = window.root?.displayName || window.Datastar?.root?.displayName;
      console.log('[passkey-plugin] Register clicked:', { email, displayName });
      await doRegister(null, email, displayName);
    });
    console.log('[passkey-plugin] Register button listener attached');
  }

  // Find login button (substring match for @passkeyLogin)
  const loginBtn = document.querySelector('button[data-on-click*="passkeyLogin"]');
  if (loginBtn) {
    loginBtn.addEventListener('click', async (e) => {
      e.preventDefault();
      console.log('[passkey-plugin] Login clicked');
      await doLogin(null);
    });
    console.log('[passkey-plugin] Login button listener attached');
  }

  console.log('[passkey-plugin] Event listeners set up successfully');
});
