import { execFileSync } from 'node:child_process';
import { readFileSync, statSync } from 'node:fs';
import { extname } from 'node:path';

const baseSha = process.env.BASE_SHA?.trim();
const maxChangedFileBytes = 10 * 1024 * 1024;
const forbiddenExtensions = new Set([
  '.bak',
  '.db',
  '.env',
  '.key',
  '.orphan',
  '.p12',
  '.pem',
  '.pfx',
  '.shm',
  '.sqlite',
  '.sqlite3',
  '.wal',
]);
const allowedEnvironmentFiles = new Set(['.env.example', '.env.sample', '.env.template']);
const secretPatterns = [
  ['private key', /-----BEGIN (?:RSA |EC |DSA |OPENSSH |PGP )?PRIVATE KEY-----/],
  ['GitHub token', /\b(?:gh[opusr]_[A-Za-z0-9]{36,}|github_pat_[A-Za-z0-9_]{80,})\b/],
  ['AWS access key', /\bAKIA[0-9A-Z]{16}\b/],
  ['OpenAI-style secret key', /\bsk-(?:proj-)?[A-Za-z0-9_-]{32,}\b/],
  ['Slack token', /\bxox[baprs]-[A-Za-z0-9-]{20,}\b/],
  ['Windows user path', /\b[A-Za-z]:\\Users\\[^\\\s]+\\/],
  ['macOS user path', /(?:^|[\s"'])\/Users\/[^/\s]+\//m],
  ['Linux user path', /(?:^|[\s"'])\/home\/[^/\s]+\//m],
];

function git(args) {
  return execFileSync('git', args, { encoding: 'utf8' }).trim();
}

function changedFiles() {
  if (baseSha) {
    return git(['diff', '--name-only', '--diff-filter=ACMR', `${baseSha}...HEAD`])
      .split(/\r?\n/)
      .filter(Boolean);
  }
  return git(['show', '--pretty=', '--name-only', 'HEAD'])
    .split(/\r?\n/)
    .filter(Boolean);
}

function modifiedHistoricalMigrations() {
  if (!baseSha) return [];
  return git([
    'diff',
    '--name-only',
    '--diff-filter=M',
    `${baseSha}...HEAD`,
    '--',
    'frontend/src-tauri/migrations/*.sql',
  ])
    .split(/\r?\n/)
    .filter(Boolean);
}

const failures = [];
const modifiedMigrations = modifiedHistoricalMigrations();
if (modifiedMigrations.length > 0) {
  failures.push(
    `Applied migration files are immutable; add a new migration instead:\n${modifiedMigrations
      .map((file) => `  - ${file}`)
      .join('\n')}`,
  );
}

for (const file of changedFiles()) {
  let stats;
  try {
    stats = statSync(file);
  } catch {
    continue;
  }
  if (!stats.isFile()) continue;

  if (stats.size > maxChangedFileBytes) {
    failures.push(`${file}: changed file is larger than 10 MiB`);
  }

  const normalized = file.replaceAll('\\', '/').toLowerCase();
  const extension = extname(normalized);
  const filename = normalized.split('/').at(-1) ?? normalized;
  const isEnvironmentFile = filename === '.env' || filename.startsWith('.env.');
  if (
    (forbiddenExtensions.has(extension) || isEnvironmentFile) &&
    !allowedEnvironmentFiles.has(filename)
  ) {
    failures.push(`${file}: database, credential, backup, or private-key files cannot be committed`);
  }

  if (stats.size === 0 || stats.size > maxChangedFileBytes) continue;
  if (/\.(?:png|jpe?g|gif|webp|ico|icns|wav|mp3|mp4|onnx|bin|dll|dylib|so|exe)$/i.test(file)) {
    continue;
  }

  let content;
  try {
    content = readFileSync(file, 'utf8');
  } catch {
    continue;
  }
  for (const [label, pattern] of secretPatterns) {
    if (pattern.test(content)) failures.push(`${file}: possible ${label}`);
  }
}

if (failures.length > 0) {
  console.error('PR policy check failed:\n');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('PR policy check passed: migrations are append-only and changed files are public-safe.');
