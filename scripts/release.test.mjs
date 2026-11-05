import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, writeFile, cp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";
import { releaseVersion, releaseConfiguration, validateChangelogs, validateUpdaterManifest, mergeUpdaterPlatforms, assertReleaseAdvance, root } from "./release.mjs";

const section = { type: "fixed", title: "Fixes", items: [{ title: "A real correction", desc: "Retains user data." }] };
const notes = () => ({ unreleased: { sections: [section] }, releases: [{ tag: "v1.2.3", name: "specX 1.2.3", date: "2026-11-04", sections: [section] }] });

test("only stable vX.Y.Z tags reach the production channel", () => {
  assert.equal(releaseVersion("v1.2.3"), "1.2.3");
  for (const tag of ["1.2.3", "v01.2.3", "v1.2.3-rc.1", "v1.2.3+local", "foundation-baseline-2026-11-04"]) assert.throws(() => releaseVersion(tag));
});

test("a delayed job cannot replace a newer published production version", () => {
  assertReleaseAdvance("v1.2.3", undefined);
  assertReleaseAdvance("v1.2.3", "v1.2.2");
  assert.throws(() => assertReleaseAdvance("v1.2.3", "v1.2.3"));
  assert.throws(() => assertReleaseAdvance("v1.2.3", "v1.3.0"));
});

test("release configuration rejects unsafe repositories and private key material", () => {
  assert.deepEqual(releaseConfiguration({ repository: "", updaterPublicKey: "" }, {}), { repository: "", publicKey: "" });
  assert.equal(releaseConfiguration({ repository: "org/repo" }, {}).repository, "org/repo");
  for (const repository of ["https://github.com/org/repo", "org/repo/releases", "org/repo?token=private", "org/repo\nnext"]) assert.throws(() => releaseConfiguration({ repository }, {}));
  assert.throws(() => releaseConfiguration({ updaterPublicKey: Buffer.from("untrusted comment: private key\nsecret").toString("base64") }, {}));
});

test("bilingual histories cannot silently drift or duplicate a release", () => {
  validateChangelogs([notes(), notes()]);
  const missing = notes();
  missing.releases = [];
  assert.throws(() => validateChangelogs([notes(), missing]));
  const duplicated = notes();
  duplicated.releases.push(duplicated.releases[0]);
  assert.throws(() => validateChangelogs([duplicated, duplicated]));
  const wrongDate = notes();
  wrongDate.releases[0].date = "2026-11-03";
  assert.throws(() => validateChangelogs([notes(), wrongDate]));
  const backwards = notes();
  backwards.releases.push({ ...backwards.releases[0], tag: "v2.0.0" });
  assert.throws(() => validateChangelogs([backwards, backwards]));
});

test("publication requires signed, fixed-version assets for all supported platforms", () => {
  const platforms = Object.fromEntries(["darwin-aarch64", "darwin-x86_64", "windows-x86_64", "linux-x86_64"].map((target) => [target, { signature: "signed", url: `https://github.com/org/repo/releases/download/v1.2.3/${target}` }]));
  const manifest = { version: "1.2.3", pub_date: "2026-11-04T00:00:00Z", platforms };
  validateUpdaterManifest(manifest, "1.2.3", "org/repo");
  assert.throws(() => validateUpdaterManifest(manifest, "1.2.4", "org/repo"));
  const missing = structuredClone(manifest);
  delete missing.platforms["darwin-aarch64"];
  assert.throws(() => validateUpdaterManifest(missing, "1.2.3", "org/repo"));
  const unsigned = structuredClone(manifest);
  unsigned.platforms["windows-x86_64"].signature = "";
  assert.throws(() => validateUpdaterManifest(unsigned, "1.2.3", "org/repo"));
  const redirected = structuredClone(manifest);
  redirected.platforms["linux-x86_64"].url = "https://other.example/package";
  assert.throws(() => validateUpdaterManifest(redirected, "1.2.3", "org/repo"));
});

test("manual macOS updater signatures merge into the release manifest", () => {
  const signed = (name) => ({ signature: "existing", url: `https://github.com/org/repo/releases/latest/download/${name}` });
  const manifest = {
    version: "1.2.3",
    pub_date: "2026-11-04T00:00:00Z",
    platforms: {
      "linux-x86_64": signed("specX.AppImage"),
      "windows-x86_64": signed("specX.msi"),
    },
  };
  mergeUpdaterPlatforms(manifest, "org/repo", {
    "darwin-aarch64": { name: "specX_aarch64.app.tar.gz", signature: "signed-arm" },
    "darwin-x86_64": { name: "specX_x64.app.tar.gz", signature: "signed-intel" },
  });
  validateUpdaterManifest(manifest, "1.2.3", "org/repo");
  assert.equal(manifest.platforms["darwin-aarch64"].url, "https://github.com/org/repo/releases/latest/download/specX_aarch64.app.tar.gz");
});

test("preparing a release synchronizes versions, seals bilingual notes and remains checkable", async () => {
  const directory = await mkdtemp(join(tmpdir(), "specx-release-"));
  try {
    for (const path of ["scripts", "apps/desktop", "src-tauri", "releases"]) await mkdir(join(directory, path), { recursive: true });
    await cp(join(root, "scripts/release.mjs"), join(directory, "scripts/release.mjs"));
    for (const path of ["package.json", "apps/desktop/package.json", "src-tauri/tauri.conf.json"]) await writeFile(join(directory, path), JSON.stringify({ version: "1.2.3" }));
    await writeFile(join(directory, "src-tauri/Cargo.toml"), '[package]\nname = "specx-desktop"\nversion = "1.2.3"\n');
    await writeFile(join(directory, "src-tauri/Cargo.lock"), '[[package]]\nname = "specx-desktop"\nversion = "1.2.3"\n');
    for (const locale of ["zh-CN", "en"]) await writeFile(join(directory, `releases/changelog.${locale}.json`), JSON.stringify(notes()));
    const run = (...args) => execFileSync(process.execPath, [join(directory, "scripts/release.mjs"), ...args], { cwd: directory, stdio: "pipe" });
    assert.throws(() => run("check", "v1.2.4"));
    run("prepare", "1.2.4");
    run("check", "v1.2.4");
    run("assets", "v1.2.4");
    const prepared = JSON.parse(await readFile(join(directory, "releases/changelog.en.json"), "utf8"));
    assert.equal(prepared.releases[0].tag, "v1.2.4");
    assert.equal(prepared.unreleased.sections.length, 0);
    assert.match(await readFile(join(directory, "src-tauri/Cargo.lock"), "utf8"), /version = "1.2.4"/);
    assert.match(await readFile(join(directory, ".local/release-assets/notes.md"), "utf8"), /English/);
    assert.throws(() => run("prepare", "1.2.4"));
    assert.throws(() => run("prepare", "1.2.2"));
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
