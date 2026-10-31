#!/usr/bin/env python3
"""Author native byte expectations from contract data, never observed output."""
import hashlib,json
from pathlib import Path
HERE=Path(__file__).resolve().parent
A=lambda counter:{'id':'A','counter':counter,'entry':True}
B=lambda counter:{'id':'B','counter':counter}
N={'items':[None,{'label':'nested'}],'marker':'LMCODE_RETURN_BEGIN\nend'}
cases=[
 ('two-reverse',['a','b'],[1,0],'builder',[A(2),B(1)],0),
 ('same-program-twice',['a','a'],[1,0],'builder',[A(2),A(1)],0),
 ('one-slot-builder',['nested'],[0],'builder',[N],0),
 ('single-nested',['nested'],[0],'single',N,0),
 ('single-list',['list'],[0],'single',[1,2],0),
 ('one-slot-builder-list',['list'],[0],'builder',[[1,2]],0),
 ('single-null',['null'],[0],'single',None,0),
 ('null-and-float',['null','float'],[1,0],'builder',[None,0.30000000000000004],0),
 ('standalone-duplicates',['a','a'],[1,0],None,[A(2),A(1)],0),
 ('one-slot-builder-failure',['error'],[0],'builder',{'ok':False,'phase':'run','code':'RUN_HOST','message':'dsl line 2:1: native entry failure','line':2,'column':1,'part':0},3),
 ('part-load-failure',['a','bad'],[0,1],'builder',{'ok':False,'phase':'load','code':'LOAD_BAD_BYTECODE','message':"not a DSL bytecode file: bad magic (expected 'DSLB')",'part':1},3),
 ('entry-failure',['a','error'],[0,1],'builder',{'ok':False,'phase':'run','code':'RUN_HOST','message':'dsl line 2:1: native entry failure','line':2,'column':1,'part':1},3),
 ('global-app-failure',['a','b'],[0,1],'builder',{'ok':False,'phase':'app','code':'APP_FAIL','message':'native validation failed'},4),
 ('single-failure',['error'],[0],'single',{'ok':False,'phase':'run','code':'RUN_HOST','message':'dsl line 2:1: native entry failure','line':2,'column':1},3),
]
bad=bytearray((HERE/'a.dslb').read_bytes());bad[0]^=1;(HERE/'bad.dslb').write_bytes(bad)
manifest={'version':1,'schema':'schema.json','cases':[]}
for name,programs,execution,mode,result,code in cases:
 config={'execution':execution}
 if name=='global-app-failure':config['app_fail']=True
 (HERE/(name+'.json')).write_text(json.dumps(config,indent=2)+'\n',encoding='utf8',newline='\n')
 envelope=result if code else {'ok':True,'result':result}
 if not code and (mode=='builder' or(mode is None and len(programs)>1)):envelope['parts']=len(programs)
 (HERE/(name+'.expect')).write_bytes(('LMCODE_RETURN_BEGIN\n'+json.dumps(envelope,ensure_ascii=False,separators=(',',':'))+'\nLMCODE_RETURN_END\n').encode())
 manifest['cases'].append({'name':name,'programs':programs,'mode':mode,'exit':code})
(HERE/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf8',newline='\n')
paths=sorted(p for p in HERE.iterdir()if p.suffix in ['.json','.lmcode','.dslb','.expect'])
(HERE/'vectors.lock').write_text(''.join(hashlib.sha256(p.read_bytes()).hexdigest()+'  '+p.name+'\n'for p in paths),encoding='utf8',newline='\n')
