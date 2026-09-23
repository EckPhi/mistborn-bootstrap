const repository = "https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap";

export function shellQuote(value) {
  if (/^[A-Za-z0-9_./:@%+=,-]+$/.test(value)) return value;
  return `'${value.replaceAll("'", `'"'"'`)}'`;
}

export function validate(config) {
  const errors = [];
  if (config.user && !/^[a-z_][a-z0-9_-]*\$?$/i.test(config.user)) errors.push("Enter a valid Linux username.");
  if (config.harden && (!Number.isInteger(config.sshPort) || config.sshPort < 1 || config.sshPort > 65535)) errors.push("SSH port must be between 1 and 65535.");
  if (config.harden && config.tcpPorts.some(port => !Number.isInteger(port) || port < 1 || port > 65535)) errors.push("Additional TCP ports must be numbers between 1 and 65535.");
  return errors;
}

export function buildCommand(config) {
  const version = /^v(\d+)\.(\d+)/.exec(config.release);
  const usesLauncher = version && (Number(version[1]) > 0 || Number(version[2]) >= 5);
  const source = usesLauncher ? "install.sh" : `dist/${config.collection}.sh`;
  const url = `${repository}/${config.release}/${source}`;
  const download = config.downloader === "wget" ? `wget -qO- ${url}` : `curl -fsSL ${url}`;
  const environment = [];
  const arguments_ = usesLauncher ? [config.collection] : [];

  if (config.collection === "server") {
    if (config.tailscaleSsh) environment.push("MISTBORN_TAILSCALE_SSH=1");
    if (config.exitNode) environment.push("MISTBORN_TAILSCALE_EXIT_NODE=1");
    if (config.tailscaleAutoUpdate) environment.push("MISTBORN_TAILSCALE_AUTO_UPDATE=1");
    if (config.rclone) environment.push("MISTBORN_RCLONE_CONFIGURE=1");
    if (config.harden) {
      environment.push("MISTBORN_HARDEN=1", `MISTBORN_SSH_PORT=${config.sshPort}`);
      if (!config.disablePassword) environment.push("MISTBORN_DISABLE_PASSWORD_AUTH=0");
      if (config.tailscaleOnly) environment.push("MISTBORN_TAILSCALE_ONLY=1");
      if (config.tcpPorts.length) environment.push(`MISTBORN_ALLOWED_TCP_PORTS=${shellQuote(config.tcpPorts.join(" "))}`);
    }
  }
  if (config.yes) arguments_.push("--yes");
  if (config.user) arguments_.push("--user", shellQuote(config.user));

  const elevated = environment.length ? `sudo env ${environment.join(" ")} bash` : "sudo bash";
  const commandArguments = arguments_.length ? ` -s -- ${arguments_.join(" ")}` : "";
  return `${download} | ${elevated}${commandArguments}`;
}

export function summarize(config) {
  const items = [config.collection === "server" ? "Install the complete server collection" : "Install shell tooling only"];
  if (config.user) items.push(`Configure shell tools for ${config.user}`);
  if (config.tailscaleSsh && config.collection === "server") items.push("Enable Tailscale SSH");
  if (config.exitNode && config.collection === "server") items.push("Advertise a Tailscale exit node");
  if (config.tailscaleAutoUpdate && config.collection === "server") items.push("Automatically install Tailscale updates");
  if (config.rclone && config.collection === "server") items.push("Open interactive rclone configuration");
  if (config.harden && config.collection === "server") {
    items.push(`Harden SSH on port ${config.sshPort} and enable UFW/fail2ban`);
    items.push(config.disablePassword ? "Disable SSH password authentication" : "Keep SSH password authentication enabled");
    if (config.tcpPorts.length) items.push(`Allow additional TCP ports: ${config.tcpPorts.join(", ")}`);
    if (config.tailscaleOnly) items.push("Restrict SSH access to Tailscale");
  }
  return items;
}

function readConfig() {
  const tcpPorts = document.querySelector("#tcp-ports").value.split(/[\s,]+/).filter(Boolean).map(Number);
  return {
    collection: document.querySelector("#collection").value,
    downloader: document.querySelector("#downloader").value,
    release: document.querySelector("#release").value,
    user: document.querySelector("#target-user").value.trim(),
    yes: document.querySelector("#yes").checked,
    tailscaleSsh: document.querySelector("#tailscale-ssh").checked,
    exitNode: document.querySelector("#exit-node").checked,
    tailscaleAutoUpdate: document.querySelector("#tailscale-auto-update").checked,
    rclone: document.querySelector("#rclone").checked,
    harden: document.querySelector("#harden").checked,
    sshPort: Number(document.querySelector("#ssh-port").value),
    tcpPorts,
    disablePassword: document.querySelector("#disable-password").checked,
    tailscaleOnly: document.querySelector("#tailscale-only").checked,
  };
}

function render() {
  const config = readConfig();
  const server = config.collection === "server";
  document.querySelector("#server-options").hidden = !server;
  document.querySelector("#hardening-options").hidden = !server || !config.harden;
  const errors = validate(config);
  document.querySelector("#command").textContent = errors.length ? "Fix the highlighted configuration to generate a command." : buildCommand(config);
  document.querySelector("#error").textContent = errors.join(" ");
  document.querySelector("#copy").disabled = errors.length > 0;
  document.querySelector("#summary").replaceChildren(...summarize(config).map(item => Object.assign(document.createElement("li"), { textContent: item })));
}

if (typeof document !== "undefined") {
  document.querySelector("#config").addEventListener("input", render);
  document.querySelector("#config").addEventListener("change", render);
  document.querySelector("#copy").addEventListener("click", async event => {
    await navigator.clipboard.writeText(document.querySelector("#command").textContent);
    event.currentTarget.textContent = "Copied";
    setTimeout(() => { event.currentTarget.textContent = "Copy install command"; }, 1200);
  });
  render();
}
