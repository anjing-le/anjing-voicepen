import assert from "node:assert/strict";
import test from "node:test";

import {
  PLATFORM_KEYS,
  ValidationError,
  parseCargoPackageVersion,
  validateManifest,
  validateTag,
  validateVersions,
} from "./validate-release.mjs";

const version = "1.2.3";
const tag = `v${version}`;

function manifest() {
  const assetNames = {
    "darwin-aarch64": "VoicePen_1.2.3_aarch64.app.tar.gz",
    "darwin-x86_64": "VoicePen_1.2.3_x64.app.tar.gz",
    "windows-x86_64": "VoicePen_1.2.3_x64-setup.exe",
  };
  return {
    version,
    notes: "- A useful change",
    pub_date: "2026-08-05T12:00:00Z",
    platforms: Object.fromEntries(
      PLATFORM_KEYS.map((platform) => [
        platform,
        {
          url: `https://github.com/anjing-le/anjing-voicepen/releases/download/${tag}/${assetNames[platform]}`,
          signature: `inline-signature-${platform}`,
        },
      ]),
    ),
  };
}

function issuesFor(callback) {
  assert.throws(callback, (error) => {
    assert.ok(error instanceof ValidationError);
    issuesFor.last = error.issues;
    return true;
  });
  return issuesFor.last;
}

test("accepts a complete stable release manifest", () => {
  assert.equal(
    validateVersions({ packageVersion: version, cargoVersion: version, tauriVersion: version }),
    version,
  );
  assert.doesNotThrow(() => validateManifest(manifest(), { version, tag }));
});

test("rejects unstable, malformed, and mismatched source versions", () => {
  for (const invalid of ["1.2", "v1.2.3", "1.2.3-rc.1", "01.2.3"]) {
    assert.ok(
      issuesFor(() =>
        validateVersions({ packageVersion: invalid, cargoVersion: invalid, tauriVersion: invalid }),
      ).some((entry) => entry.includes("stable SemVer")),
    );
  }
  assert.ok(
    issuesFor(() =>
      validateVersions({ packageVersion: version, cargoVersion: "1.2.4", tauriVersion: version }),
    ).some((entry) => entry.startsWith("version:")),
  );
  assert.ok(
    issuesFor(() =>
      validateVersions({ packageVersion: version, cargoVersion: version, tauriVersion: "1.3.0" }),
    ).some((entry) => entry.startsWith("version:")),
  );
});

test("Cargo parser reads package version rather than dependency versions", () => {
  assert.equal(
    parseCargoPackageVersion('[package]\nname = "app"\nversion = "1.2.3"\n\n[dependencies]\ncrate = { version = "9" }'),
    version,
  );
  assert.throws(() => parseCargoPackageVersion('[dependencies]\nfoo = { version = "1" }'));
});

test("rejects manifest version drift and v-prefixed versions", () => {
  for (const manifestVersion of ["1.2.4", "v1.2.3"]) {
    const value = manifest();
    value.version = manifestVersion;
    assert.ok(issuesFor(() => validateManifest(value, { version, tag })).some((entry) => entry.includes("latest.json.version")));
  }
});

test("requires a tag that exactly matches the stable version", () => {
  for (const invalid of [undefined, "", "1.2.3", "v1.2.4", "v1.2.3-rc.1"]) {
    assert.ok(issuesFor(() => validateTag(invalid, version)).some((entry) => entry.startsWith("tag:")));
  }
  assert.doesNotThrow(() => validateTag(tag, version));
});

test("rejects missing, extra, and misspelled platform keys", () => {
  const missing = manifest();
  delete missing.platforms["darwin-aarch64"];
  assert.ok(issuesFor(() => validateManifest(missing, { version, tag })).some((entry) => entry.includes("darwin-aarch64")));

  for (const key of ["linux-x86_64", "windows-aarch64", "darwin-arm64"]) {
    const extra = manifest();
    extra.platforms[key] = extra.platforms["darwin-aarch64"];
    assert.ok(issuesFor(() => validateManifest(extra, { version, tag })).some((entry) => entry.includes(key)));
  }
});

test("rejects mutable, cross-version, cross-repository, and unsafe URLs", () => {
  const cases = [
    "https://github.com/anjing-le/anjing-voicepen/releases/latest/download/app.zip",
    "https://github.com/anjing-le/anjing-voicepen/releases/download/v1.2.4/app.zip",
    `https://github.com/someone/else/releases/download/${tag}/app.zip`,
    `http://github.com/anjing-le/anjing-voicepen/releases/download/${tag}/app.zip`,
    `https://example.com/anjing-le/anjing-voicepen/releases/download/${tag}/app.zip`,
    `https://github.com/anjing-le/anjing-voicepen/releases/download/${tag}/app.zip?token=x`,
    `https://github.com/anjing-le/anjing-voicepen/releases/download/${tag}/app.zip#fragment`,
    `https://github.com:444/anjing-le/anjing-voicepen/releases/download/${tag}/app.zip`,
  ];
  for (const url of cases) {
    const value = manifest();
    value.platforms["darwin-aarch64"].url = url;
    assert.ok(issuesFor(() => validateManifest(value, { version, tag })).some((entry) => entry.includes("darwin-aarch64.url")));
  }
});

test("rejects empty, nested, encoded-slash, and duplicate asset URLs", () => {
  for (const suffix of ["", "nested/app.app.tar.gz", "nested%2Fapp.app.tar.gz", ".", ".."]) {
    const value = manifest();
    value.platforms["darwin-aarch64"].url = `https://github.com/anjing-le/anjing-voicepen/releases/download/${tag}/${suffix}`;
    assert.ok(issuesFor(() => validateManifest(value, { version, tag })).some((entry) => entry.includes("darwin-aarch64.url")));
  }
  const duplicate = manifest();
  duplicate.platforms["windows-x86_64"].url = duplicate.platforms["darwin-x86_64"].url;
  assert.ok(issuesFor(() => validateManifest(duplicate, { version, tag })).some((entry) => entry.includes("duplicates")));
});

test("enforces updater asset types for every platform key", () => {
  const wrongExtensions = {
    "darwin-aarch64": "VoicePen_1.2.3_x64.app.tar.gz",
    "darwin-x86_64": "VoicePen_1.2.3_aarch64.app.tar.gz",
    "windows-x86_64": "VoicePen_1.2.3_aarch64.app.tar.gz",
  };
  for (const [platform, filename] of Object.entries(wrongExtensions)) {
    const value = manifest();
    value.platforms[platform].url = `https://github.com/anjing-le/anjing-voicepen/releases/download/${tag}/${filename}`;
    assert.ok(
      issuesFor(() => validateManifest(value, { version, tag })).some((entry) =>
        entry.includes(`platforms.${platform}.url`),
      ),
    );
  }

  const caseInsensitive = manifest();
  caseInsensitive.platforms["darwin-aarch64"].url =
    `https://github.com/anjing-le/anjing-voicepen/releases/download/${tag}/VoicePen_AARCH64.APP.TAR.GZ`;
  caseInsensitive.platforms["darwin-x86_64"].url =
    `https://github.com/anjing-le/anjing-voicepen/releases/download/${tag}/VoicePen_X64.APP.TAR.GZ`;
  caseInsensitive.platforms["windows-x86_64"].url =
    `https://github.com/anjing-le/anjing-voicepen/releases/download/${tag}/VoicePen_X64-SETUP.EXE`;
  assert.doesNotThrow(() => validateManifest(caseInsensitive, { version, tag }));
});

test("requires inline non-empty signatures", () => {
  for (const signature of ["", "   ", "https://github.com/signature.sig", "HTTP://example.com/sig"]) {
    const value = manifest();
    value.platforms["windows-x86_64"].signature = signature;
    assert.ok(issuesFor(() => validateManifest(value, { version, tag })).some((entry) => entry.includes("signature")));
  }
});

test("requires user-visible notes and a valid publication date", () => {
  const noNotes = manifest();
  noNotes.notes = " ";
  assert.ok(issuesFor(() => validateManifest(noNotes, { version, tag })).some((entry) => entry.includes("notes")));
  const badDate = manifest();
  badDate.pub_date = "not-a-date";
  assert.ok(issuesFor(() => validateManifest(badDate, { version, tag })).some((entry) => entry.includes("pub_date")));
  const looseDate = manifest();
  looseDate.pub_date = "2026-08-05";
  assert.ok(issuesFor(() => validateManifest(looseDate, { version, tag })).some((entry) => entry.includes("pub_date")));
});

test("limits notes to safe plain text accepted by the application", () => {
  const accepted = manifest();
  accepted.notes = "新增\t功能\n修复问题";
  assert.doesNotThrow(() => validateManifest(accepted, { version, tag }));

  const tooLong = manifest();
  tooLong.notes = "声".repeat(8_001);
  assert.ok(issuesFor(() => validateManifest(tooLong, { version, tag })).some((entry) => entry.includes("8000")));

  for (const control of ["\r", "\0", "\u001b", "\u007f", "\u0085"]) {
    const value = manifest();
    value.notes = `说明${control}内容`;
    assert.ok(issuesFor(() => validateManifest(value, { version, tag })).some((entry) => entry.includes("control")));
  }
});
