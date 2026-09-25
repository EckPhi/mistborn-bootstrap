const repository = "https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap";
const releaseInstallers = new Set();
const selectionFields = {
  collection: ["collection", "select"],
  downloader: ["downloader", "select"],
  release: ["release", "select"],
  user: ["target-user", "text"],
  yes: ["yes", "checkbox"],
  tailscaleSsh: ["tailscale-ssh", "checkbox"],
  exitNode: ["exit-node", "checkbox"],
  tailscaleAutoUpdate: ["tailscale-auto-update", "checkbox"],
  rclone: ["rclone", "checkbox"],
  harden: ["harden", "checkbox"],
  sshPort: ["ssh-port", "number"],
  tcpPorts: ["tcp-ports", "text"],
  disablePassword: ["disable-password", "checkbox"],
  tailscaleOnly: ["tailscale-only", "checkbox"],
  plexUfw: ["plex-ufw", "checkbox"],
  plexTailscale: ["plex-tailscale", "checkbox"],
  plexLanCidr: ["plex-lan-cidr", "text"],
};

export function selectionFromURL(search, allowedReleases) {
  const params = new URLSearchParams(search);
  const value = (key, fallback) => params.has(key) ? params.get(key) : fallback;
  const collection = value("collection", "server");
  const downloader = value("downloader", "curl");
  const release = value("release", allowedReleases[0] ?? "v0.5.6");
  const tcpPortsText = value("tcpPorts", "");
  return {
    collection: ["server", "shell"].includes(collection) ? collection : "server",
    downloader: ["curl", "wget"].includes(downloader) ? downloader : "curl",
    release: allowedReleases.includes(release) ? release : (allowedReleases[0] ?? "v0.5.6"),
    user: value("user", ""),
    yes: value("yes", "0") === "1",
    tailscaleSsh: value("tailscaleSsh", "0") === "1",
    exitNode: value("exitNode", "0") === "1",
    tailscaleAutoUpdate: value("tailscaleAutoUpdate", "0") === "1",
    rclone: value("rclone", "0") === "1",
    harden: value("harden", "0") === "1",
    sshPort: Number(value("sshPort", "22")),
    tcpPorts: tcpPortsText.split(/[\s,]+/).filter(Boolean).map(Number),
    tcpPortsText,
    disablePassword: value("disablePassword", "1") === "1",
    tailscaleOnly: value("tailscaleOnly", "0") === "1",
    plexUfw: value("plexUfw", "0") === "1",
    plexTailscale: value("plexTailscale", "0") === "1",
    plexLanCidr: value("plexLanCidr", ""),
  };
}

export function selectionToSearch(config) {
  const params = new URLSearchParams();
  for (const key of Object.keys(selectionFields)) {
    const value = key === "tcpPorts" ? config.tcpPortsText : config[key];
    params.set(key, key === "tcpPorts" ? (value ?? "") : typeof value === "boolean" ? (value ? "1" : "0") : String(value));
  }
  return params.toString();
}

export function shellQuote(value) {
  if (/^[A-Za-z0-9_./:@%+=,-]+$/.test(value)) return value;
  return `'${value.replaceAll("'", `'"'"'`)}'`;
}

export function validate(config) {
  const errors = [];
  if (config.user && !/^[a-z_][a-z0-9_-]*\$?$/i.test(config.user)) errors.push("Enter a valid Linux username.");
  if (config.harden && (!Number.isInteger(config.sshPort) || config.sshPort < 1 || config.sshPort > 65535)) errors.push("SSH port must be between 1 and 65535.");
  if (config.harden && config.tcpPorts.some(port => !Number.isInteger(port) || port < 1 || port > 65535)) errors.push("Additional TCP ports must be numbers between 1 and 65535.");
  if (config.harden && config.plexUfw && config.plexLanCidr && !validIPv4Cidr(config.plexLanCidr)) errors.push("Plex LAN CIDR must be an IPv4 network such as 192.168.1.0/24.");
  return errors;
}

function validIPv4Cidr(value) {
  const [address, prefix, extra] = value.split("/");
  if (extra !== undefined || !/^\d+$/.test(prefix ?? "") || Number(prefix) > 32) return false;
  const octets = address.split(".");
  return octets.length === 4 && octets.every(octet => /^\d+$/.test(octet) && Number(octet) <= 255);
}

export function buildCommand(config, releaseInstallerAvailable = false) {
  const version = /^v(\d+)\.(\d+)/.exec(config.release);
  const usesLauncher = version && (Number(version[1]) > 0 || Number(version[2]) >= 5);
  const source = usesLauncher ? "install.sh" : `dist/${config.collection}.sh`;
  const url = releaseInstallerAvailable
    ? `https://github.com/EckPhi/mistborn-bootstrap/releases/download/${config.release}/install.sh`
    : `${repository}/${config.release}/${source}`;
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
      if (config.plexUfw) {
        environment.push("MISTBORN_PLEX_UFW=1");
        if (config.plexTailscale) environment.push("MISTBORN_PLEX_TAILSCALE=1");
        if (config.plexLanCidr) environment.push(`MISTBORN_PLEX_LAN_CIDR=${shellQuote(config.plexLanCidr)}`);
      }
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
    if (config.plexUfw) {
      items.push("Allow Plex remote access on TCP 32400");
      if (config.plexTailscale) items.push("Allow Plex local services through tailscale0");
      if (config.plexLanCidr) items.push(`Allow Plex discovery and DLNA from ${config.plexLanCidr}`);
    }
  }
  return items;
}

function readConfig() {
  const tcpPortsText = document.querySelector("#tcp-ports").value;
  const tcpPorts = tcpPortsText.split(/[\s,]+/).filter(Boolean).map(Number);
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
    tcpPortsText,
    disablePassword: document.querySelector("#disable-password").checked,
    tailscaleOnly: document.querySelector("#tailscale-only").checked,
    plexUfw: document.querySelector("#plex-ufw").checked,
    plexTailscale: document.querySelector("#plex-tailscale").checked,
    plexLanCidr: document.querySelector("#plex-lan-cidr").value.trim(),
  };
}

function render() {
  const config = readConfig();
  const server = config.collection === "server";
  document.querySelector("#server-options").hidden = !server;
  document.querySelector("#hardening-options").hidden = !server || !config.harden;
  const errors = validate(config);
  document.querySelector("#command").textContent = errors.length ? "Fix the highlighted configuration to generate a command." : buildCommand(config, releaseInstallers.has(config.release));
  document.querySelector("#error").textContent = errors.join(" ");
  document.querySelector("#copy").disabled = errors.length > 0;
  document.querySelector("#summary").replaceChildren(...summarize(config).map(item => Object.assign(document.createElement("li"), { textContent: item })));
  const query = selectionToSearch(config);
  const nextURL = `${location.pathname}?${query}${location.hash}`;
  if (`${location.pathname}${location.search}${location.hash}` !== nextURL) history.replaceState(null, "", nextURL);
}

if (typeof document !== "undefined") {
  const releaseSelect = document.querySelector("#release");
  const initialize = async () => {
    try {
      const response = await fetch("./releases.json", { cache: "no-cache" });
      if (response.ok) {
        const catalog = await response.json();
        if (Array.isArray(catalog) && catalog.length) {
          releaseSelect.replaceChildren(...catalog.map(release => {
            if (release.installer) releaseInstallers.add(release.tag);
            return Object.assign(document.createElement("option"), {
              value: release.tag,
              textContent: release.tag,
            });
          }));
        }
      }
    } catch {
      // Keep the checked-in release list as a usable offline fallback.
    }
    const releases = [...releaseSelect.options].map(option => option.value);
    const initial = selectionFromURL(location.search, releases);
    for (const [key, [id, type]] of Object.entries(selectionFields)) {
      const input = document.querySelector(`#${id}`);
      if (type === "checkbox") input.checked = initial[key];
      else if (key === "tcpPorts") input.value = initial.tcpPortsText;
      else input.value = String(initial[key]);
    }
    document.querySelector("#config").addEventListener("input", render);
    document.querySelector("#config").addEventListener("change", render);
    document.querySelector("#copy").addEventListener("click", async event => {
      await navigator.clipboard.writeText(document.querySelector("#command").textContent);
      event.currentTarget.textContent = "Copied";
      setTimeout(() => { event.currentTarget.textContent = "Copy install command"; }, 1200);
    });
    render();
  };
  initialize();
}
