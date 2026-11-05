import { readFile, writeFile, mkdir } from "node:fs/promises";
import { realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

export const root = fileURLToPath(new URL("../", import.meta.url));
const locales = ["zh-CN", "en"];
const stableVersion = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const readJson = async (path) => JSON.parse(await readFile(resolve(root, path), "utf8"));
const writeJson = (path, value) => writeFile(resolve(root, path), `${JSON.stringify(value, null, 2)}\n`);
const compareVersions = (left, right) => {
  const a = left.split(".").map(BigInt);
  const b = right.split(".").map(BigInt);
  for (let index = 0; index < 3; index++) if (a[index] !== b[index]) return a[index] < b[index] ? -1 : 1;
  return 0;
};

export function releaseVersion(tag) {
  if (!/^v/.test(tag) || !stableVersion.test(tag.slice(1))) throw new Error("Use a stable release tag: vX.Y.Z (no prerelease or build suffix).");
  return tag.slice(1);
}

export function assertReleaseAdvance(tag, currentTag) {
  const version = releaseVersion(tag);
  if (currentTag && compareVersions(version, releaseVersion(currentTag)) <= 0) throw new Error("A delayed release must not replace the same or a newer published version.");
}

export function releaseConfiguration(config, env = process.env) {
  const repository = (env.SPECX_RELEASE_REPOSITORY || config.repository || "").trim();
  const publicKey = (env.SPECX_UPDATER_PUBLIC_KEY || config.updaterPublicKey || "").trim();
  if (repository && !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) throw new Error("Release repository must be owner/repository.");
  if (publicKey) {
    const decoded = Buffer.from(publicKey, "base64").toString("utf8");
    const lines = decoded.trim().split(/\r?\n/);
    const key = Buffer.from(lines[1] || "", "base64");
    if (lines.length !== 2 || !lines[0].startsWith("untrusted comment:") || key.length !== 42 || key.subarray(0, 2).toString() !== "Ed") throw new Error("Use the full contents of the Tauri updater .pub file, not a private key.");
  }
  return { repository, publicKey };
}

export function validateChangelogs(documents) {
  const tags = documents[0].releases.map((release) => release.tag);
  if (new Set(tags).size !== tags.length) throw new Error("Duplicate changelog tag.");
  for (const document of documents) {
    if (JSON.stringify(document.releases.map((release) => release.tag)) !== JSON.stringify(tags)) throw new Error("Both languages must contain the same releases in the same order.");
    for (const release of [document.unreleased, ...document.releases]) {
      if (release.tag) {
        releaseVersion(release.tag);
        if (!/^\d{4}-\d{2}-\d{2}$/.test(release.date) || !Number.isFinite(Date.parse(release.date))) throw new Error("Release date must be YYYY-MM-DD.");
      }
      if (!Array.isArray(release.sections) || (release.tag && !release.sections.length)) throw new Error("Write release notes in both languages before preparing a release.");
      for (const section of release.sections) {
        if (!["added", "improved", "fixed", "changed", "removed"].includes(section.type) || !section.title || !Array.isArray(section.items) || !section.items.length || section.items.some((item) => typeof item.title !== "string" || !item.title.trim() || typeof item.desc !== "string"))
          throw new Error("Invalid changelog section or item.");
      }
    }
  }
  for (let index = 0; index < tags.length; index++) {
    if (index && compareVersions(releaseVersion(tags[index - 1]), releaseVersion(tags[index])) <= 0) throw new Error("Changelog releases must be in descending version order.");
    if (documents.some((doc) => doc.releases[index].date !== documents[0].releases[index].date)) throw new Error("Release dates must match across languages.");
  }
}

function notesMarkdown(release) {
  return release.sections.map((section) => `### ${section.title}\n\n${section.items.map((item) => `- ${item.title}${item.desc ? ` — ${item.desc}` : ""}`).join("\n")}`).join("\n\n");
}

export async function checkRelease(tag) {
  const pkg = await readJson("package.json");
  const app = await readJson("apps/desktop/package.json");
  const tauri = await readJson("src-tauri/tauri.conf.json");
  const cargo = await readFile(resolve(root, "src-tauri/Cargo.toml"), "utf8");
  const cargoVersion = cargo.match(/^version = "([^"]+)"/m)?.[1];
  if (!stableVersion.test(pkg.version) || [app.version, tauri.version, cargoVersion].some((value) => value !== pkg.version)) throw new Error("Root, frontend, Tauri config, and desktop Cargo versions must match.");
  const lock = await readFile(resolve(root, "src-tauri/Cargo.lock"), "utf8");
  if (lock.match(/name = "specx-desktop"\r?\nversion = "([^"]+)"/)?.[1] !== pkg.version) throw new Error("Desktop Cargo.lock must match the application version.");
  const documents = await Promise.all(locales.map((locale) => readJson(`releases/changelog.${locale}.json`)));
  validateChangelogs(documents);
  if (documents[0].releases.length && compareVersions(pkg.version, releaseVersion(documents[0].releases[0].tag)) < 0) throw new Error("The application version cannot precede its published history.");
  if (tag) {
    if (releaseVersion(tag) !== pkg.version) throw new Error("Tag does not match application version.");
    if (documents.some((document) => document.releases[0]?.tag !== tag)) throw new Error("The tagged release must be the first entry in both changelogs.");
  }
  return { version: pkg.version, documents };
}

async function prepareRelease(version) {
  releaseVersion(`v${version}`);
  const { version: current, documents } = await checkRelease();
  if (compareVersions(version, current) < 0) throw new Error("A release cannot decrease the current application version.");
  if (documents.some((doc) => doc.releases.some((release) => release.tag === `v${version}`))) throw new Error("This version already has release notes; published releases are immutable.");
  if (documents.some((doc) => !doc.unreleased.sections.length)) throw new Error("Write Unreleased notes in both languages before preparing a release.");
  const date = new Date().toISOString().slice(0, 10);
  for (const document of documents) {
    document.releases.unshift({ tag: `v${version}`, name: `specX ${version}`, date, sections: document.unreleased.sections });
    document.unreleased = { sections: [] };
  }
  const jsonPaths = ["package.json", "apps/desktop/package.json", "src-tauri/tauri.conf.json"];
  for (const path of jsonPaths) {
    const json = await readJson(path);
    json.version = version;
    await writeJson(path, json);
  }
  const cargoPath = resolve(root, "src-tauri/Cargo.toml");
  await writeFile(cargoPath, (await readFile(cargoPath, "utf8")).replace(/^version = "[^"]+"/m, `version = "${version}"`));
  const lockPath = resolve(root, "src-tauri/Cargo.lock");
  await writeFile(
    lockPath,
    (await readFile(lockPath, "utf8")).replace(/(name = "specx-desktop"\r?\nversion = ")[^"]+("\r?\n)/, (_, before, after) => `${before}${version}${after}`),
  );
  for (let index = 0; index < locales.length; index++) await writeJson(`releases/changelog.${locales[index]}.json`, documents[index]);
}

export async function writeUpdaterConfiguration(required = false) {
  const config = releaseConfiguration(await readJson("release.config.json"));
  const enabled = Boolean(config.repository && config.publicKey);
  if (required && !enabled) throw new Error("A release requires SPECX_RELEASE_REPOSITORY and SPECX_UPDATER_PUBLIC_KEY (or release.config.json values).");
  const overlay = {
    bundle: { createUpdaterArtifacts: enabled },
    plugins: { updater: { pubkey: config.publicKey, endpoints: enabled ? [`https://github.com/${config.repository}/releases/latest/download/latest.json`] : [] } },
  };
  await mkdir(resolve(root, ".local"), { recursive: true });
  const path = resolve(root, ".local/tauri-update.conf.json");
  await writeFile(path, JSON.stringify(overlay, null, 2));
  return path;
}

async function writeReleaseAssets(tag) {
  const { documents } = await checkRelease(tag);
  const output = resolve(root, ".local/release-assets");
  await mkdir(output, { recursive: true });
  for (let index = 0; index < locales.length; index++) await writeFile(resolve(output, `releases-${locales[index]}.json`), JSON.stringify({ updatedAt: new Date().toISOString(), releases: documents[index].releases }, null, 2));
  await writeFile(resolve(output, "notes.md"), `## 简体中文\n\n${notesMarkdown(documents[0].releases[0])}\n\n## English\n\n${notesMarkdown(documents[1].releases[0])}\n`);
}

export function validateUpdaterManifest(manifest, version, repository) {
  if (manifest.version !== version || !Number.isFinite(Date.parse(manifest.pub_date))) throw new Error("Invalid updater version or publish date.");
  const allowedPrefixes = [`https://github.com/${repository}/releases/download/v${version}/`, `https://github.com/${repository}/releases/latest/download/`];
  for (const platform of ["darwin-aarch64", "darwin-x86_64", "windows-x86_64", "linux-x86_64"]) {
    const asset = manifest.platforms?.[platform];
    if (!asset || typeof asset.signature !== "string" || !asset.signature.trim() || !allowedPrefixes.some((prefix) => asset.url?.startsWith(prefix))) throw new Error(`Missing or invalid signed update artifact: ${platform}`);
  }
}

export function mergeUpdaterPlatforms(manifest, repository, platforms) {
  for (const [platform, artifact] of Object.entries(platforms)) {
    if (!artifact || typeof artifact.name !== "string" || typeof artifact.signature !== "string" || !artifact.signature.trim()) throw new Error(`Missing signed updater artifact: ${platform}`);
    manifest.platforms ??= {};
    manifest.platforms[platform] = {
      signature: artifact.signature,
      url: `https://github.com/${repository}/releases/latest/download/${artifact.name}`,
    };
  }
  return manifest;
}

async function main() {
  const [command, argument] = process.argv.slice(2);
  if (command === "check") await checkRelease(argument);
  else if (command === "prepare") await prepareRelease(argument);
  else if (command === "config") await writeUpdaterConfiguration(argument === "--required");
  else if (command === "assets") await writeReleaseAssets(argument);
  else if (command === "verify-manifest") {
    const { repository } = releaseConfiguration(await readJson("release.config.json"));
    const { version } = await checkRelease(process.env.GITHUB_REF_NAME);
    validateUpdaterManifest(JSON.parse(await readFile(argument, "utf8")), version, repository);
  } else throw new Error("Usage: release.mjs check [vX.Y.Z] | prepare X.Y.Z | config [--required] | assets vX.Y.Z | verify-manifest <path>");
}

const scriptPath = fileURLToPath(import.meta.url);
if (process.argv[1] && realpathSync(process.argv[1]) === realpathSync(scriptPath))
  main().catch((error) => {
    console.error(error.message);
    process.exitCode = 1;
  });
