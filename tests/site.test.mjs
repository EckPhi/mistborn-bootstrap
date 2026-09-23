import assert from "node:assert/strict";
import test from "node:test";
import { buildCommand, shellQuote, summarize, validate } from "../site/app.mjs";

const defaults = {
  collection: "server", downloader: "curl", release: "v0.5.0", user: "", yes: false,
  tailscaleSsh: false, exitNode: false, tailscaleAutoUpdate: false, rclone: false, harden: false, sshPort: 22,
  tcpPorts: [], disablePassword: true, tailscaleOnly: false,
};

test("builds the minimal pinned command", () => {
  assert.equal(buildCommand(defaults), "curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.5.0/install.sh | sudo bash -s -- server");
});

test("adds supported server configuration with safe quoting", () => {
  const config = { ...defaults, user: "phil", yes: true, tailscaleSsh: true, tailscaleAutoUpdate: true, harden: true, sshPort: 2222, tcpPorts: [80, 443], tailscaleOnly: true };
  assert.equal(buildCommand(config), "curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.5.0/install.sh | sudo env MISTBORN_TAILSCALE_SSH=1 MISTBORN_TAILSCALE_AUTO_UPDATE=1 MISTBORN_HARDEN=1 MISTBORN_SSH_PORT=2222 MISTBORN_TAILSCALE_ONLY=1 MISTBORN_ALLOWED_TCP_PORTS='80 443' bash -s -- server --yes --user phil");
  assert.ok(summarize(config).some(item => item.includes("port 2222")));
  assert.ok(summarize(config).includes("Automatically install Tailscale updates"));
});

test("never applies server settings to the shell collection", () => {
  const command = buildCommand({ ...defaults, collection: "shell", harden: true, tailscaleSsh: true, tailscaleAutoUpdate: true });
  assert.equal(command, "curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.5.0/install.sh | sudo bash -s -- shell");
});

test("keeps pre-launcher releases on their bundled shell scripts", () => {
  const command = buildCommand({ ...defaults, release: "v0.4.0", yes: true });
  assert.equal(command, "curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.4.0/dist/server.sh | sudo bash -s -- --yes");
});

test("validates usernames and every configured port", () => {
  assert.deepEqual(validate({ ...defaults, user: "bad user", harden: true, sshPort: 0, tcpPorts: [443, 70000] }), [
    "Enter a valid Linux username.", "SSH port must be between 1 and 65535.", "Additional TCP ports must be numbers between 1 and 65535.",
  ]);
  assert.equal(shellQuote("80 443"), "'80 443'");
});
