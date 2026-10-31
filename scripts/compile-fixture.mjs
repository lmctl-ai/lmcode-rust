// Compiler-library tooling for native fixtures; no script-visible bytecode export.
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL} from 'node:url';
const [input,output]=process.argv.slice(2);
if(!input||!output||!process.env.LMCODE_COMPILER_MODULE)throw Error('set LMCODE_COMPILER_MODULE to the compiler library and supply source/output');
const source=fs.readFileSync(input,'utf8');
const header=/^.*= _lmcode_ schema="([^"]+)";\r?\n/m.exec(source);
if(!header)throw Error('fixture requires a literal file schema');
const body=source.slice(header.index+header[0].length).split(/^end _lmcode_\s*$/m)[0].trimEnd();
const {parseDslSchema,compileDsl}=await import(pathToFileURL(process.env.LMCODE_COMPILER_MODULE).href);
const schema=path.resolve(path.dirname(input),header[1]);
fs.mkdirSync(path.dirname(output),{recursive:true});
fs.writeFileSync(output,compileDsl(body.split('\n'),parseDslSchema(fs.readFileSync(schema),schema)).bytes);
