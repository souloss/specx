import { readFile } from "node:fs/promises";

const files = ["core/src/persistence.rs", "core/src/postgres.rs"];
for (const file of files) {
  const source = await readFile(file, "utf8");
  const production = source.split("#[cfg(test)]", 1)[0];
  const latestMatch = production.match(/LATEST_SCHEMA_VERSION:\s*i64\s*=\s*(\d+)/) ?? (file.endsWith("postgres.rs") ? (await readFile("core/src/persistence.rs", "utf8")).match(/LATEST_SCHEMA_VERSION:\s*i64\s*=\s*(\d+)/) : null);
  if (!latestMatch) throw new Error(`${file}: missing LATEST_SCHEMA_VERSION`);
  const latest = Number(latestMatch[1]);
  const versions = [...production.matchAll(/schema_migrations[\s\S]{0,180}?VALUES\s*\(\s*(\d+)/g)].map((match) => Number(match[1]));
  const expected = Array.from({ length: latest }, (_, index) => index + 1);
  const unique = [...new Set(versions)].sort((a, b) => a - b);
  if (unique.length !== expected.length || unique.some((value, index) => value !== expected[index])) {
    throw new Error(`${file}: migration markers must cover 1..${latest}; found ${unique.join(", ")}`);
  }
  console.log(`${file}: schema migration markers 1..${latest}`);
}
