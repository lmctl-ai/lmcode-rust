// Compiler-library tooling for native fixtures; no script-visible bytecode export.
import fs from 'node:fs';
import path from 'node:path';
import {pathToFileURL, fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
const [input,output]=process.argv.slice(2);
if(!input||!output)throw Error('supply source/output');
if(!process.env.LMCODE_COMPILER_MODULE) {
  // Published CLI bundles do not expose the compiler-library API. Reuse a
  // compiler-generated fixture only while source/schema/bytecode hashes agree.
  const root=fileURLToPath(new URL('../',import.meta.url));
  const fixtures=[
    {source:'examples/rule-entries/rules.lms',manifest:'fixtures/rule-entries.json',output:'fixtures/rule-entries.lmcode'},
    {source:'dslvm/tests/fixtures/rule-methods/program.lms',manifest:'fixtures/rule-methods.json',output:'dslvm/tests/fixtures/rule-methods/program.dslb'},
  ];
  const fixture=fixtures.find(item=>path.resolve(input)===path.join(root,item.source));
  if(!fixture)throw Error('set LMCODE_COMPILER_MODULE for this fixture');
  const hashes=JSON.parse(fs.readFileSync(path.join(root,fixture.manifest),'utf8'));
  for(const [file,expected] of Object.entries(hashes)) {
    const actual=createHash('sha256').update(fs.readFileSync(path.join(root,file))).digest('hex');
    if(actual!==expected)throw Error('stale compiler fixture: '+file);
  }
  fs.mkdirSync(path.dirname(output),{recursive:true});
  fs.copyFileSync(path.join(root,fixture.output),output);
} else {
const source=fs.readFileSync(input,'utf8');
const header=/^.*= _lmcode_ schema="([^"]+)";\r?\n/m.exec(source);
if(!header)throw Error('fixture requires a literal file schema');
const body=source.slice(header.index+header[0].length).split(/^end _lmcode_\s*$/m)[0].trimEnd();
const headerLine=source.slice(0,header.index).split(/\r?\n/).length;
const sourceMapper=(lineIndex,column)=>({line:headerLine+1+lineIndex,column});
const {parseDslSchema,compileDsl}=await import(pathToFileURL(process.env.LMCODE_COMPILER_MODULE).href);
const schema=path.resolve(path.dirname(input),header[1]);
fs.mkdirSync(path.dirname(output),{recursive:true});
fs.writeFileSync(output,compileDsl(body.split('\n'),parseDslSchema(fs.readFileSync(schema),schema),sourceMapper).bytes);

}
