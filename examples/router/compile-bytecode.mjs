import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [monoLmscript, sourcePath, schemaPath, outputPath] = process.argv.slice(2);
assert.ok(monoLmscript && sourcePath && schemaPath && outputPath, 'usage: compile-bytecode.mjs MONO_LMSCRIPT SOURCE SCHEMA OUTPUT');
const { compileDsl, parseDslSchema } = await import(pathToFileURL(join(monoLmscript, 'dsl.ts')).href);
const source = readFileSync(sourcePath, 'utf8');
const block = /_lmcode_\s+schema\s*=.*?;\s*\r?\n([\s\S]*?)\r?\nend _lmcode_/.exec(source);
const body = (block ? block[1] : source).replace(/\r\n/g, '\n').trimEnd().split('\n');
const schemaBytes = readFileSync(schemaPath);
const schema = parseDslSchema(schemaBytes, schemaPath);
const compiled = compileDsl(body, schema);
writeFileSync(outputPath, compiled.bytes);
