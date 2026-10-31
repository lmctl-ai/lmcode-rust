#!/usr/bin/env python3
"""Compile only the new result fixtures; older corpora/cutover belong to slice 6."""
import hashlib, json, os, subprocess, sys, tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
HERE=Path(__file__).resolve().parent
canonical=json.loads((ROOT/'std/lmcode-std.schema.json').read_text())
schema={**canonical,'name':'return_vectors','interfaces':{'Entry':{'params':[],'returns':'int'}}}
schema['functions']['entry_fail']=[{'params':[],'returns':'void'}]
schema['functions']['expect_result']=[{'params':['string'],'returns':'void'}]
(HERE/'schema.json').write_text(json.dumps(schema,indent=2)+'\n',encoding='utf8',newline='\n')
manifest={'schema':'schema.json','version':6,'host':{'entries':'call in declared order; repeated-calls calls twice; int HALT results must remain 7','frames':'emit only after body, entries and application validation succeed; no frame on any error'},'vectors':[]}
for name in ['float-roundtrip','entry-sink','entry-error','repeated-calls','unset-null','all-overloads','unsafe-json-integer','scalar-field','invalid-utf8']:
 manifest['vectors'].append({'name':name,'source':name+'.dsl','bytecode':name+'.dslb','expect':{'file':name+'.expect','exit':3 if name in ['entry-error','unsafe-json-integer','scalar-field','invalid-utf8'] else 0},'repeat':2 if name=='repeated-calls' else 1})
(HERE/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf8',newline='\n')
with tempfile.TemporaryDirectory(prefix='return-compile-') as temporary:
 for case in manifest['vectors']:
  src=HERE/case['source'];binary=HERE/case['bytecode'];wrapper=Path(temporary)/'compile.lms'
  wrapper.write_text('p = _lmcode_ schema='+json.dumps(str(HERE/'schema.json'))+';\n'+src.read_text(encoding='utf8')+'end _lmcode_\nsavedsl(p,'+json.dumps(str(binary))+');\n',encoding='utf8',newline='\n')
  subprocess.run([os.environ.get('LMSCRIPT_BIN','lmscript'),str(wrapper)],check=True)
paths=[HERE/'schema.json',HERE/'manifest.json']+[HERE/case[key] for case in manifest['vectors'] for key in ['source','bytecode']]+[HERE/case['expect']['file'] for case in manifest['vectors']]
(HERE/'vectors.lock').write_text(''.join(hashlib.sha256(path.read_bytes()).hexdigest()+'  '+path.name+'\n' for path in sorted(paths)),encoding='utf8',newline='\n')
