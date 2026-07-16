import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { cpSync, mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

const policySource = new URL('./pr-policy-check.mjs', import.meta.url);

function runGit(cwd, args) {
  return execFileSync('git', args, { cwd, encoding: 'utf8' }).trim();
}

function createRepository() {
  const root = mkdtempSync(join(tmpdir(), 'briefli-policy-'));
  mkdirSync(join(root, '.github', 'scripts'), { recursive: true });
  mkdirSync(join(root, 'frontend', 'src-tauri', 'migrations'), { recursive: true });
  cpSync(policySource, join(root, '.github', 'scripts', 'pr-policy-check.mjs'));
  writeFileSync(
    join(root, 'frontend', 'src-tauri', 'migrations', '20260101000000_initial.sql'),
    'CREATE TABLE example (id TEXT PRIMARY KEY);\n',
  );
  writeFileSync(join(root, 'README.md'), '# Fixture\n');
  runGit(root, ['init', '--initial-branch=main']);
  runGit(root, ['config', 'user.email', 'ci@example.invalid']);
  runGit(root, ['config', 'user.name', 'CI Test']);
  runGit(root, ['add', '.']);
  runGit(root, ['commit', '-m', 'baseline']);
  return root;
}

function commit(root, message = 'candidate') {
  runGit(root, ['add', '.']);
  runGit(root, ['commit', '-m', message]);
}

function policy(root) {
  const result = spawnSync(process.execPath, ['.github/scripts/pr-policy-check.mjs'], {
    cwd: root,
    encoding: 'utf8',
    env: { ...process.env, BASE_SHA: `${runGit(root, ['rev-parse', 'HEAD~1'])}` },
  });
  return { code: result.status, output: `${result.stdout}${result.stderr}` };
}

function withRepository(callback) {
  const root = createRepository();
  try {
    callback(root);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test('accepts a new append-only migration', () => {
  withRepository((root) => {
    writeFileSync(
      join(root, 'frontend', 'src-tauri', 'migrations', '20260102000000_add_name.sql'),
      'ALTER TABLE example ADD COLUMN name TEXT;\n',
    );
    commit(root);
    assert.equal(policy(root).code, 0);
  });
});

test('rejects a modified historical migration', () => {
  withRepository((root) => {
    writeFileSync(
      join(root, 'frontend', 'src-tauri', 'migrations', '20260101000000_initial.sql'),
      'CREATE TABLE changed (id TEXT PRIMARY KEY);\n',
    );
    commit(root);
    const result = policy(root);
    assert.notEqual(result.code, 0);
    assert.match(result.output, /migration files are immutable/i);
  });
});

test('rejects a deleted historical migration', () => {
  withRepository((root) => {
    rmSync(join(root, 'frontend', 'src-tauri', 'migrations', '20260101000000_initial.sql'));
    commit(root);
    const result = policy(root);
    assert.notEqual(result.code, 0);
    assert.match(result.output, /migration files are immutable/i);
  });
});

test('rejects a renamed historical migration', () => {
  withRepository((root) => {
    const migrations = join(root, 'frontend', 'src-tauri', 'migrations');
    runGit(root, [
      'mv',
      join(migrations, '20260101000000_initial.sql'),
      join(migrations, '20260101000001_initial.sql'),
    ]);
    commit(root);
    const result = policy(root);
    assert.notEqual(result.code, 0);
    assert.match(result.output, /migration files are immutable/i);
  });
});

test('rejects a migration added and then modified later in the range', () => {
  withRepository((root) => {
    const addedMigration = join(
      root,
      'frontend',
      'src-tauri',
      'migrations',
      '20260102000000_add_name.sql',
    );
    writeFileSync(addedMigration, 'ALTER TABLE example ADD COLUMN name TEXT;\n');
    commit(root, 'add migration');
    const baseSha = runGit(root, ['rev-parse', 'HEAD~1']);
    writeFileSync(addedMigration, 'ALTER TABLE example ADD COLUMN changed TEXT;\n');
    commit(root, 'modify added migration');
    const result = spawnSync(process.execPath, ['.github/scripts/pr-policy-check.mjs'], {
      cwd: root,
      encoding: 'utf8',
      env: { ...process.env, BASE_SHA: baseSha },
    });
    const output = `${result.stdout}${result.stderr}`;
    assert.notEqual(result.status, 0);
    assert.match(output, /migration files are immutable/i);
  });
});

test('rejects local databases and private keys', () => {
  withRepository((root) => {
    writeFileSync(join(root, 'meeting_minutes.sqlite'), 'not a real database');
    const privateKeyMarker = ['-----BEGIN ', 'PRIVATE KEY-----'].join('');
    writeFileSync(join(root, 'secret.txt'), `${privateKeyMarker}\nsecret\n`);
    commit(root);
    const result = policy(root);
    assert.notEqual(result.code, 0);
    assert.match(result.output, /cannot be committed/i);
    assert.match(result.output, /possible private key/i);
  });
});

test('rejects environment files, backups, WAL files, and certificate bundles', () => {
  withRepository((root) => {
    writeFileSync(join(root, '.env.local'), 'TOKEN=fake\n');
    writeFileSync(join(root, 'meeting.sqlite-wal'), 'wal');
    writeFileSync(join(root, 'meeting.bak'), 'backup');
    writeFileSync(join(root, 'signing.pfx'), 'certificate');
    commit(root);
    const result = policy(root);
    assert.notEqual(result.code, 0);
    assert.match(result.output, /cannot be committed/i);
  });
});

test('rejects machine-specific user paths', () => {
  withRepository((root) => {
    const windowsUserPath = ['C:', 'Users', 'someone', 'Briefli', 'data'].join('\\');
    writeFileSync(join(root, 'config.txt'), `path=${windowsUserPath}\n`);
    commit(root);
    const result = policy(root);
    assert.notEqual(result.code, 0);
    assert.match(result.output, /possible Windows user path/i);
  });
});
