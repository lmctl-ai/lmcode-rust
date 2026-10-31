#!/usr/bin/env python3
"""Compile dry-dispatch faults and require semantic sentinel/dummy assertions to catch them."""
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile

ROOT=Path(__file__).resolve().parents[1]
INVOCATION=ROOT/'scripts/test-invocation-vectors.py'
IGNORE=shutil.ignore_patterns('.git','target','build','__pycache__')

with tempfile.TemporaryDirectory(prefix='rust-dry-callback-controls-') as temporary:
    work=Path(temporary)/'repo'
    shutil.copytree(ROOT,work,ignore=IGNORE)
    source=work/'dslvm/src/lib.rs'
    std_source=work/'dslvm/src/stdlib.rs'
    original=source.read_text()
    original_std=std_source.read_text()
    binary=work/'target/release/invocation-conformance'
    env=os.environ.copy()

    def build():
        result=subprocess.run(['cargo','build','--release','-p','dsl-conformance','--bin','invocation-conformance','--locked','--offline'],cwd=work,env=env,capture_output=True,text=True)
        if result.returncode:
            raise AssertionError(('mutant build failed',result.stdout,result.stderr))

    def invocation_check(label):
        test_env=dict(env,LMCODE_INVOCATION_HARNESS=str(binary))
        result=subprocess.run([sys.executable,str(work/'scripts/test-invocation-vectors.py')],cwd=work,env=test_env,capture_output=True,text=True)
        if result.returncode!=1 or 'AssertionError' not in result.stderr:
            raise AssertionError((label,result.returncode,result.stdout,result.stderr))
        print(f'CAUGHT {label}',flush=True)

    build()
    result=subprocess.run([sys.executable,str(work/'scripts/test-invocation-vectors.py')],cwd=work,env=dict(env,LMCODE_INVOCATION_HARNESS=str(binary)),capture_output=True,text=True)
    if result.returncode:
        raise AssertionError(('dry control baseline failed',result.stdout,result.stderr))
    print('PASS dry callback mutation baseline',flush=True)

    fallback='None => stdlib::dry_dummy(ctx, &signature.returns)?,'
    assert original.count(fallback)==1,('fallback-to-real anchor',original.count(fallback))
    source.write_text(original.replace(fallback,'None => real_fn(ctx, &args)?,',1))
    build();invocation_check('fallback to real callback')

    source.write_text(original)
    callback='Some(DryFunctionSlot::Callback(callback)) => callback(ctx, &args)?,'
    assert original.count(callback)==1,('double-call anchor',original.count(callback))
    doubled='Some(DryFunctionSlot::Callback(callback)) => { let first = callback(ctx, &args); let _second = callback(ctx, &args); first? },'
    source.write_text(original.replace(callback,doubled,1))
    build();invocation_check('double preview callback')

    source.write_text(original)
    fresh='"IntList" => Value::object("IntList", ctx.insert_collection("IntList", collections::IntList::default())?),'
    shared='''"IntList" => {
            let handle = if let Some(handle) = ctx.lookup("__shared_dry_int_list") { handle } else {
                let handle = ctx.insert_collection("IntList", collections::IntList::default())?;
                ctx.set("__shared_dry_int_list", handle);
                handle
            };
            Value::object("IntList", handle)
        },'''
    assert original_std.count(fresh)==1,('shared-dummy anchor',original_std.count(fresh))
    std_source.write_text(original_std.replace(fresh,shared,1))
    result=subprocess.run(['cargo','test','-p','dslvm','--test','dry_callbacks','--locked','--offline'],cwd=work,env=env,capture_output=True,text=True)
    combined=result.stdout+result.stderr
    if result.returncode==0 or 'test result: FAILED' not in combined or 'error[E' in combined:
        raise AssertionError(('shared mutable dummy',result.returncode,result.stdout,result.stderr))
    print('CAUGHT shared mutable dummy',flush=True)
