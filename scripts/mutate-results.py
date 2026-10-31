#!/usr/bin/env python3
"""Production-copy controls for invocation results; compilation errors never count."""
from pathlib import Path
import os, re, shutil, subprocess, tempfile
ROOT=Path(__file__).resolve().parent.parent
CASES=[
 ('registration removed','dslvm/src/stdlib.rs','    register_result(registry)?;',''),
 ('implicit missing sink','dslvm/src/stdlib/result.rs','return Err(Error::host("lmcode return: no result sink"));','ctx.result_sink = Some(ResultSink::default());'),
 ('failed write clears prior value','dslvm/src/stdlib/result.rs','let owned = convert(ctx, value, "")?;','ctx.result_sink.as_ref().unwrap().reset().unwrap(); let owned = convert(ctx, value, "")?;'),
 ('source copies discarded','dslvm/src/stdlib/result.rs','state.value = owned;','state.value = OwnedJson::Null;'),
 ('legacy float rounding','dslvm/src/stdlib/result.rs','canonical_float(n)','crate::format_g15(n)'),
 ('raw token inspection omitted','dslvm/src/stdlib/result.rs','number_token(&n.raw, path)?','float(n.float, path)?'),
 ('underflow accepted','dslvm/src/stdlib/result.rs','if n == 0.0 && !digits.is_empty() {','if false {'),
 ('unsafe integral float accepted','dslvm/src/stdlib/result.rs','if n.fract() == 0.0 && n.abs() > 9007199254740991.0 {','if false {'),
 ('duplicates merged into output','dslvm/src/stdlib/result.rs','if !seen.insert(k) {','if false && !seen.insert(k) {'),
 ('arena clears result','dslvm/src/lib.rs','pub(crate) fn clear_collections(&mut self) {','pub(crate) fn clear_collections(&mut self) { self.result_sink = Some(stdlib::result::ResultSink::default());'),
 ('missing seal','dslvm/src/stdlib/result.rs','state.sealed=true;','state.sealed=false;'),
 ('reuse slot sink','dslvm/src/stdlib/invocation.rs','let slot=self.slot(index)?;\n  if slot.sink.is_none()', 'let slot=self.slot(0)?;\n  if slot.sink.is_none()'),
 ('reset active sink','dslvm/src/stdlib/invocation.rs','let sink=slot.sink.as_ref().unwrap();if sink.sealed()', 'let sink=slot.sink.as_ref().unwrap();sink.reset()?;if sink.sealed()'),
 ('wrong part','dslvm/src/stdlib/invocation.rs','part:if self.builder{index}else{None}', 'part:if self.builder{Some(0)}else{None}'),
 ('omit flush','dslvm/src/stdlib/invocation.rs','out.flush()', 'Ok(())'),
 ('failure last wins','dslvm/src/stdlib/invocation.rs','if self.failure.is_some(){return Ok(());}', 'if false{return Ok(())}'),
 ('omit runtime contract','dslvm/src/stdlib/result.rs','state.contract.validate(&owned, "")?;', ''),
 ('allow post failure start','dslvm/src/stdlib/invocation.rs','if failed{return Err(', 'if false && failed{return Err('),
 ('ignore metadata index','dslvm/src/stdlib/invocation.rs','if member(p,"index").and_then(node_integer)!=Some(i as i64)', 'if false'),
 ('ignore registration duplicate','dslvm/src/stdlib/invocation.rs','if seen[index]{return Err(metadata("duplicate registration"));}', ''),

]
with tempfile.TemporaryDirectory(prefix='rust-result-mutants-') as temp:
 dest=Path(temp)/'repo'
 shutil.copytree(ROOT,dest,ignore=shutil.ignore_patterns('.git','target'))
 env=dict(os.environ,CARGO_TARGET_DIR=str(Path(temp)/'target'))
 command=['cargo','test','-p','dslvm','--lib','--test','result_sink','--test','return_vectors','--test','invocation','--locked']
 def run(label,mutant):
  result=subprocess.run(command,cwd=dest,env=env,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
  (ROOT/'target').mkdir(exist_ok=True)
  (ROOT/'target'/('result-mutant-'+label.replace(' ','-')+'.log')).write_text(result.stdout)
  if not mutant:
   assert result.returncode==0,result.stdout
  else:
   assert result.returncode==101 and 'test result: FAILED' in result.stdout and 'panicked at' in result.stdout and 'error: could not compile' not in result.stdout and 'process didn\'t exit successfully' not in result.stdout,result.stdout
  print(('CAUGHT ' if mutant else 'PASS ')+label,flush=True)
 run('baseline',False)
 for label,file,before,after in CASES:
  path=dest/file;original=path.read_text()
  tokens=re.findall(r'"(?:\\.|[^"\\])*"|[A-Za-z_]\w*|[0-9]+|[^\s]',before)
  pattern=r'\s*'.join(re.escape(t) for t in tokens)
  matches=list(re.finditer(pattern,original));assert len(matches)==1,(label,'anchor must be unique',len(matches))
  match=matches[0];path.write_text(original[:match.start()]+after+original[match.end():])
  try:run(label,True)
  finally:path.write_text(original)
 print('PASS all '+str(len(CASES))+' result mutants caught')
