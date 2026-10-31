#!/usr/bin/env python3
"""Real single-process native executions, private per-slot files and byte goldens."""
import hashlib,json,os,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1];HERE=ROOT/'conformance/invocation-vectors'
BINARY=Path(os.environ.get('LMCODE_INVOCATION_HARNESS',str(ROOT/'target/release/invocation-conformance'))).resolve()
manifest=json.loads((HERE/'manifest.json').read_text())
for row in (HERE/'vectors.lock').read_text().splitlines():
 digest,name=row.split('  ');assert hashlib.sha256((HERE/name).read_bytes()).hexdigest()==digest,name
for case in manifest['cases']:
 with tempfile.TemporaryDirectory(prefix='native-invocation-')as temporary:
  folder=Path(temporary);parts=[];files=[]
  for index,name in enumerate(case['programs']):
   raw=(HERE/(name+'.dslb')).read_bytes()
   if name=='bad':
    original=(HERE/'a.dslb').read_bytes();assert raw[0]==original[0]^1 and raw[1:]==original[1:]
   else:assert raw[:6]==b'DSLB\0\6' and raw[6:38]==hashlib.sha256((HERE/'schema.json').read_bytes()).digest()
   target=folder/f'part {index}-水.dslb';target.write_bytes(raw);files.append(str(target));parts.append({'index':index,'file':str(target)})
  env=dict(os.environ);env.pop('LMCODE_INVOCATION_FILE',None)
  if case['mode'] is not None:
   path=folder/'invocation.json';path.write_text(json.dumps({'version':1,'mode':case['mode'],'parts':parts}));env['LMCODE_INVOCATION_FILE']=str(path)
  else:files=[files[0]]*len(files) # standalone duplicate file occurrences are legal
  # Reverse parsed argv order too: canonical indices must come from metadata.
  parsed=files[::-1]if case['mode']is not None else files
  result=subprocess.run([str(BINARY),str(HERE/'schema.json'),str(HERE/(case['name']+'.json')),*parsed],env=env,capture_output=True)
  expected=(HERE/(case['name']+'.expect')).read_bytes()
  assert(result.returncode,result.stdout,result.stderr)==(case['exit'],expected,b''),(case['name'],result)
  assert result.stdout.count(b'LMCODE_RETURN_BEGIN\n')==1 and result.stdout.endswith(b'LMCODE_RETURN_END\n')
 print('PASS invocation golden',case['name'])
print('PASS',len(manifest['cases']),'native invocation goldens')
# Private metadata is validated before any program dispatch, not treated as a
# source of contracts or a reason to guess argument order.
with tempfile.TemporaryDirectory(prefix='metadata-controls-')as temporary:
 folder=Path(temporary);a=folder/'a.dslb';b=folder/'b.dslb'
 a.write_bytes((HERE/'float.dslb').read_bytes());b.write_bytes(a.read_bytes())
 base={'version':1,'mode':'single','parts':[{'index':0,'file':str(a)}]}
 variants=[]
 def add(name,fn):
  value=json.loads(json.dumps(base));fn(value);variants.append((name,json.dumps(value).encode(),[str(a)]))
 add('version',lambda v:v.update(version=2))
 add('mode',lambda v:v.update(mode='batch'))
 add('empty',lambda v:v.update(parts=[]))
 add('unknown-root',lambda v:v.update(result={'type':'int'}))
 add('unknown-part',lambda v:v['parts'][0].update(contract='int'))
 add('relative',lambda v:v['parts'][0].update(file='relative.dslb'))
 add('nul',lambda v:v['parts'][0].update(file=str(a)+'\0'))
 add('gap',lambda v:v['parts'][0].update(index=1))
 add('negative',lambda v:v['parts'][0].update(index=-1))
 add('fractional',lambda v:v['parts'][0].update(index=0.5))
 add('unsafe',lambda v:v['parts'][0].update(index=9007199254740993))
 duplicate={'version':1,'mode':'builder','parts':[{'index':0,'file':str(a)},{'index':1,'file':str(a)}]}
 variants.append(('duplicate-files',json.dumps(duplicate).encode(),[str(a),str(a)]))
 two={'version':1,'mode':'builder','parts':[{'index':0,'file':str(a)},{'index':1,'file':str(b)}]}
 variants.append(('duplicate-registration',json.dumps(two).encode(),[str(a),str(a)]))
 variants.append(('missing-registration',json.dumps(two).encode(),[str(a)]))
 variants.append(('extra-registration',json.dumps(base).encode(),[str(a),str(b)]))
 two['mode']='single';variants.append(('single-count',json.dumps(two).encode(),[str(a),str(b)]))
 variants.append(('duplicate-key',json.dumps(base).replace('{','{"version":1,',1).encode(),[str(a)]))
 variants.append(('bom',b'\xef\xbb\xbf'+json.dumps(base).encode(),[str(a)]))
 variants.append(('utf8',b'\xff',[str(a)]))
 for name,raw,parsed in variants:
  path=folder/'invocation.json';path.write_bytes(raw)
  env=dict(os.environ,LMCODE_INVOCATION_FILE=str(path))
  r=subprocess.run([str(BINARY),str(HERE/'schema.json'),str(HERE/'single-null.json'),*parsed],capture_output=True,env=env)
  assert r.returncode==2 and r.stderr==b'',(name,r)
  lines=r.stdout.splitlines();assert len(lines)==3 and lines[0]==b'LMCODE_RETURN_BEGIN' and lines[2]==b'LMCODE_RETURN_END',(name,r)
  value=json.loads(lines[1]);assert set(value)=={'ok','phase','code','message'} and value['ok']is False and value['phase']=='cli' and value['code']=='CLI_USAGE',(name,value)
 print('PASS',len(variants),'malformed private metadata controls')
 for malformed in ['', 'relative.json']:
  r=subprocess.run([str(BINARY),str(HERE/'schema.json'),str(HERE/'single-null.json'),str(a)],capture_output=True,env=dict(os.environ,LMCODE_INVOCATION_FILE=malformed))
  assert r.returncode==2 and b'"result"'not in r.stdout
 print('PASS present empty/relative metadata path never falls back')
 # Number tokens denoting safe integers follow JSON's mathematical integer
 # semantics; no bool/fractional/unsafe value is accepted.
 base['version']=1.0;base['parts'][0]['index']=0.0
 path=folder/'invocation.json';path.write_text(json.dumps(base))
 r=subprocess.run([str(BINARY),str(HERE/'schema.json'),str(HERE/'single-null.json'),str(a)],capture_output=True,env=dict(os.environ,LMCODE_INVOCATION_FILE=str(path)))
 assert r.returncode==0 and json.loads(r.stdout.splitlines()[1])=={'ok':True,'result':0.30000000000000004},r
 print('PASS safe integral metadata number tokens')

if os.name != 'nt':
 env=dict(os.environ);env.pop('LMCODE_INVOCATION_FILE',None)
 r=subprocess.run([str(BINARY),str(HERE/'schema.json'),str(HERE/'single-null.json'),b'/invalid-\xff'],env=env,capture_output=True)
 assert r.returncode==2 and json.loads(r.stdout.splitlines()[1])['code']=='CLI_USAGE',r
 print('PASS invalid standalone UTF-8 rejected before filesystem access')
