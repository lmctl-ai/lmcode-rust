#!/usr/bin/env python3
"""Production-copy controls for invocation results; compilation errors never count."""
from pathlib import Path
import os, shutil, subprocess, tempfile
ROOT=Path(__file__).resolve().parent.parent
CASES=[
 ('registration removed','dslvm/src/stdlib.rs','    register_result(registry)?;',''),
 ('implicit missing sink','dslvm/src/stdlib/result.rs','return Err(Error::host("lmcode return: no result sink"));','ctx.result_sink = Some(ResultSink::default());'),
 ('failed write clears prior value','dslvm/src/stdlib/result.rs','let owned = convert(ctx, value, "")?;','ctx.result_sink.as_mut().unwrap().reset(); let owned = convert(ctx, value, "")?;'),
 ('source copies discarded','dslvm/src/stdlib/result.rs','ctx.result_sink.as_mut().unwrap().value = owned;','ctx.result_sink.as_mut().unwrap().value = OwnedJson::Null;'),
 ('legacy float rounding','dslvm/src/stdlib/result.rs','format!("{n:?}").trim_end_matches(".0").to_owned()','crate::format_g15(n)'),
 ('raw token inspection omitted','dslvm/src/stdlib/result.rs','number_token(&n.raw, path)?','float(n.float, path)?'),
 ('underflow accepted','dslvm/src/stdlib/result.rs','if n == 0.0 && !digits.is_empty() {','if false {'),
 ('unsafe integral float accepted','dslvm/src/stdlib/result.rs','if n.fract() == 0.0 && n.abs() > 9007199254740991.0 {','if false {'),
 ('duplicates merged into output','dslvm/src/stdlib/result.rs','if !seen.insert(k) {','if false && !seen.insert(k) {'),
 ('failed invocation framed','dslvm/src/stdlib/result.rs','success.then(|| self.frame())','Some(self.frame())'),
 ('arena clears result','dslvm/src/lib.rs','pub(crate) fn clear_collections(&mut self) {','pub(crate) fn clear_collections(&mut self) { self.result_sink = Some(stdlib::result::ResultSink::default());'),
]
with tempfile.TemporaryDirectory(prefix='rust-result-mutants-') as temp:
 dest=Path(temp)/'repo'
 shutil.copytree(ROOT,dest,ignore=shutil.ignore_patterns('.git','target'))
 env=dict(os.environ,CARGO_TARGET_DIR=str(Path(temp)/'target'))
 command=['cargo','test','-p','dslvm','--lib','--test','result_sink','--test','return_vectors','--locked']
 def run(label,mutant):
  result=subprocess.run(command,cwd=dest,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  (ROOT/'target').mkdir(exist_ok=True)
  (ROOT/'target'/('result-mutant-'+label.replace(' ','-')+'.log')).write_text(result.stdout)
  if not mutant:
   assert result.returncode==0,result.stdout
  else:
   assert result.returncode!=0 and 'test result: FAILED' in result.stdout and 'error: could not compile' not in result.stdout,result.stdout
  print(('CAUGHT ' if mutant else 'PASS ')+label,flush=True)
 run('baseline',False)
 for label,file,before,after in CASES:
  path=dest/file;original=path.read_text();assert original.count(before)==1,(label,'anchor must be unique')
  path.write_text(original.replace(before,after))
  try:run(label,True)
  finally:path.write_text(original)
 print('PASS all '+str(len(CASES))+' result mutants caught')
