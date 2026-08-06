import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

export const RELEASE_SCOPES = {
  all: ["darwin-aarch64", "darwin-x86_64", "windows-x86_64"],
  macos: ["darwin-aarch64", "darwin-x86_64"],
};
export const PLATFORM_KEYS = RELEASE_SCOPES.all;

const STABLE_SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const RFC3339_TIMESTAMP =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/u;
const RELEASE_PATH_PREFIX = "/anjing-le/anjing-voicepen/releases/download/";
const MAX_NOTES_CHARACTERS = 8_000;
const DISALLOWED_NOTES_CONTROL = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/u;
const ASSET_SUFFIXES = {
  "darwin-aarch64": "_aarch64.app.tar.gz",
  "darwin-x86_64": "_x64.app.tar.gz",
  "windows-x86_64": "_x64-setup.exe",
};

export class ValidationError extends Error {
  constructor(issues) {
    super(issues.join("\n"));
    this.name = "ValidationError";
    this.issues = issues;
  }
}

function issue(field, message) {
  return `${field}: ${message}`;
}

function parseJson(raw, field) {
  try {
    return JSON.parse(raw);
  } catch {
    throw new ValidationError([issue(field, "must contain valid JSON")]);
  }
}

export function parseCargoPackageVersion(raw) {
  let inPackage = false;
  for (const sourceLine of raw.split(/\r?\n/u)) {
    const line = sourceLine.replace(/\s+#.*$/u, "").trim();
    const section = line.match(/^\[([^\]]+)\]$/u);
    if (section) {
      inPackage = section[1].trim() === "package";
      continue;
    }
    if (!inPackage) continue;
    const version = line.match(/^version\s*=\s*"([^"]+)"\s*$/u);
    if (version) return version[1];
  }
  throw new ValidationError([
    issue("src-tauri/Cargo.toml package.version", "is missing"),
  ]);
}

function validateStableVersion(version, field, issues) {
  if (typeof version !== "string" || !STABLE_SEMVER.test(version)) {
    issues.push(issue(field, "must be a stable SemVer in x.y.z form"));
  }
}

export function validateVersions({
  packageVersion,
  cargoVersion,
  tauriVersion,
}) {
  const issues = [];
  validateStableVersion(packageVersion, "package.json.version", issues);
  validateStableVersion(
    cargoVersion,
    "src-tauri/Cargo.toml package.version",
    issues,
  );
  validateStableVersion(
    tauriVersion,
    "src-tauri/tauri.conf.json.version",
    issues,
  );
  if (new Set([packageVersion, cargoVersion, tauriVersion]).size !== 1) {
    issues.push(
      issue(
        "version",
        "package.json, Cargo.toml, and tauri.conf.json must match",
      ),
    );
  }
  if (issues.length) throw new ValidationError(issues);
  return packageVersion;
}

export function validateTag(tag, version) {
  if (!tag)
    throw new ValidationError([
      issue("tag", "is required via --tag or GITHUB_REF_NAME"),
    ]);
  if (tag !== `v${version}`)
    throw new ValidationError([issue("tag", `must equal v${version}`)]);
}

function validateAssetUrl(value, platform, tag, issues) {
  const field = `platforms.${platform}.url`;
  if (typeof value !== "string" || value.trim() === "") {
    issues.push(issue(field, "must be a non-empty URL"));
    return null;
  }
  let url;
  try {
    url = new URL(value);
  } catch {
    issues.push(issue(field, "must be a valid URL"));
    return null;
  }
  if (url.protocol !== "https:" || url.hostname !== "github.com") {
    issues.push(issue(field, "must use https://github.com"));
  }
  if (url.port || url.username || url.password || url.search || url.hash) {
    issues.push(
      issue(
        field,
        "must not include a port, credentials, query parameters, or fragments",
      ),
    );
  }
  const expectedPrefix = `${RELEASE_PATH_PREFIX}${tag}/`;
  if (!url.pathname.startsWith(expectedPrefix)) {
    issues.push(
      issue(
        field,
        `must reference the immutable ${tag} release in anjing-le/anjing-voicepen`,
      ),
    );
    return value;
  }
  const encodedAsset = url.pathname.slice(expectedPrefix.length);
  let asset = encodedAsset;
  try {
    asset = decodeURIComponent(encodedAsset);
  } catch {
    issues.push(issue(field, "contains invalid URL encoding"));
    return value;
  }
  if (!asset || asset.includes("/") || asset === "." || asset === "..") {
    issues.push(issue(field, "must end with one non-empty asset filename"));
  } else if (
    !asset.toLocaleLowerCase("en-US").endsWith(ASSET_SUFFIXES[platform])
  ) {
    issues.push(
      issue(
        field,
        `must reference a ${ASSET_SUFFIXES[platform]} updater asset`,
      ),
    );
  }
  return value;
}

export function validateManifest(manifest, { version, tag, scope = "all" }) {
  const issues = [];
  const requiredPlatforms = RELEASE_SCOPES[scope];
  if (!requiredPlatforms) {
    throw new ValidationError([
      issue(
        "scope",
        `must be one of: ${Object.keys(RELEASE_SCOPES).join(", ")}`,
      ),
    ]);
  }
  if (!manifest || typeof manifest !== "object" || Array.isArray(manifest)) {
    throw new ValidationError([issue("latest.json", "must be a JSON object")]);
  }
  if (manifest.version !== version) {
    issues.push(
      issue("latest.json.version", `must equal ${version} without a v prefix`),
    );
  }
  if (typeof manifest.notes !== "string" || manifest.notes.trim() === "") {
    issues.push(issue("latest.json.notes", "must be a non-empty string"));
  } else {
    if ([...manifest.notes].length > MAX_NOTES_CHARACTERS) {
      issues.push(
        issue(
          "latest.json.notes",
          `must not exceed ${MAX_NOTES_CHARACTERS} Unicode characters`,
        ),
      );
    }
    if (DISALLOWED_NOTES_CONTROL.test(manifest.notes)) {
      issues.push(
        issue(
          "latest.json.notes",
          "must not contain control characters other than tab or newline",
        ),
      );
    }
  }
  if (
    typeof manifest.pub_date !== "string" ||
    manifest.pub_date.trim() === "" ||
    !RFC3339_TIMESTAMP.test(manifest.pub_date) ||
    Number.isNaN(Date.parse(manifest.pub_date))
  ) {
    issues.push(
      issue("latest.json.pub_date", "must be a valid ISO/RFC3339 timestamp"),
    );
  }
  const platforms = manifest.platforms;
  if (!platforms || typeof platforms !== "object" || Array.isArray(platforms)) {
    issues.push(issue("latest.json.platforms", "must be an object"));
  } else {
    const keys = Object.keys(platforms);
    for (const platform of requiredPlatforms) {
      if (!Object.hasOwn(platforms, platform)) {
        issues.push(issue(`platforms.${platform}`, "is required"));
      }
    }
    for (const platform of keys) {
      if (!requiredPlatforms.includes(platform)) {
        issues.push(
          issue(
            `platforms.${platform}`,
            `is not in the ${scope} release matrix`,
          ),
        );
      }
    }
    const urls = new Map();
    for (const platform of requiredPlatforms) {
      const entry = platforms[platform];
      if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
        if (Object.hasOwn(platforms, platform)) {
          issues.push(issue(`platforms.${platform}`, "must be an object"));
        }
        continue;
      }
      const url = validateAssetUrl(entry.url, platform, tag, issues);
      if (url) {
        if (urls.has(url)) {
          issues.push(
            issue(`platforms.${platform}.url`, `duplicates ${urls.get(url)}`),
          );
        } else {
          urls.set(url, platform);
        }
      }
      const signature = entry.signature;
      if (typeof signature !== "string" || signature.trim() === "") {
        issues.push(
          issue(
            `platforms.${platform}.signature`,
            "must be a non-empty inline signature",
          ),
        );
      } else if (/^https?:\/\//iu.test(signature.trim())) {
        issues.push(
          issue(
            `platforms.${platform}.signature`,
            "must be inline data, not a URL",
          ),
        );
      }
    }
  }
  if (issues.length) throw new ValidationError(issues);
}

export async function validateRepository({
  root,
  manifestPath,
  tag,
  scope = "all",
  versionsOnly = false,
}) {
  const [packageRaw, cargoRaw, tauriRaw] = await Promise.all([
    readFile(path.join(root, "package.json"), "utf8"),
    readFile(path.join(root, "src-tauri/Cargo.toml"), "utf8"),
    readFile(path.join(root, "src-tauri/tauri.conf.json"), "utf8"),
  ]);
  const packageJson = parseJson(packageRaw, "package.json");
  const tauriJson = parseJson(tauriRaw, "src-tauri/tauri.conf.json");
  const version = validateVersions({
    packageVersion: packageJson.version,
    cargoVersion: parseCargoPackageVersion(cargoRaw),
    tauriVersion: tauriJson.version,
  });
  if (versionsOnly) return version;
  validateTag(tag, version);
  const manifestRaw = await readFile(
    path.resolve(root, manifestPath ?? "latest.json"),
    "utf8",
  );
  validateManifest(parseJson(manifestRaw, "latest.json"), {
    version,
    tag,
    scope,
  });
  return version;
}

function parseArguments(argv) {
  const options = {
    root: process.cwd(),
    manifestPath: "latest.json",
    tag: process.env.GITHUB_REF_NAME,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--versions-only") options.versionsOnly = true;
    else if (
      argument === "--root" ||
      argument === "--manifest" ||
      argument === "--tag" ||
      argument === "--scope"
    ) {
      const value = argv[index + 1];
      if (!value || value.startsWith("--"))
        throw new Error(`${argument} requires a value`);
      index += 1;
      if (argument === "--root") options.root = path.resolve(value);
      if (argument === "--manifest") options.manifestPath = value;
      if (argument === "--tag") options.tag = value;
      if (argument === "--scope") options.scope = value;
    } else throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

const isMain =
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMain) {
  try {
    const options = parseArguments(process.argv.slice(2));
    const version = await validateRepository(options);
    console.log(
      options.versionsOnly
        ? `versions valid: ${version}`
        : `release metadata valid: v${version}`,
    );
  } catch (error) {
    if (error instanceof ValidationError) {
      for (const entry of error.issues) console.error(`- ${entry}`);
      process.exitCode = 1;
    } else {
      console.error(error instanceof Error ? error.message : String(error));
      process.exitCode = 2;
    }
  }
}
