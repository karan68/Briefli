import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import ts from 'typescript';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'bun:test';

const modulePath = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  '..',
  'src',
  'lib',
  'summary-template.ts'
);
const require = createRequire(import.meta.url);

const source = fs.readFileSync(modulePath, 'utf8');
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2020,
  },
}).outputText;
const module = { exports: {} };
vm.runInNewContext(compiled, { exports: module.exports, module, require });

const { createUniqueTemplateId, moveArrayItem, slugifyTemplateId } = module.exports;

describe('summary template helpers', () => {
  test('slugifies names into safe ids', () => {
    expect(slugifyTemplateId(' Client Call (EMEA) ')).toBe('client-call-emea');
    expect(slugifyTemplateId('---')).toBe('');
    expect(slugifyTemplateId('A'.repeat(80))).toHaveLength(64);
  });

  test('creates collision-free ids without exceeding 64 characters', () => {
    expect(createUniqueTemplateId('Client Call', [])).toBe('client-call');
    expect(createUniqueTemplateId('Client Call', ['client-call', 'client-call-2'])).toBe(
      'client-call-3'
    );
    expect(createUniqueTemplateId('A'.repeat(80), ['a'.repeat(64)])).toBe(
      `${'a'.repeat(62)}-2`
    );
  });

  test('reorders immutably and ignores invalid positions', () => {
    const original = ['summary', 'decisions', 'actions'];
    expect(Array.from(moveArrayItem(original, 2, 0))).toEqual(['actions', 'summary', 'decisions']);
    expect(Array.from(moveArrayItem(original, 0, 1))).toEqual(['decisions', 'summary', 'actions']);
    expect(Array.from(moveArrayItem(original, -1, 2))).toEqual(original);
    expect(original).toEqual(['summary', 'decisions', 'actions']);
  });
});
