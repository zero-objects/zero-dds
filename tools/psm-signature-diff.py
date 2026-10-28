#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""Compares the member functions of the classes in the normative headers of DDS-PSM-Cxx 1.0
(the OMG's `dds/` tree) with those of `crates/cpp/include/dds`.

    python3 tools/psm-signature-diff.py <omg hpp/dds directory> [more class names]

The headers of ours are preprocessed first (`clang++ -E -P`), so that the members the policy and
status macros declare are visible. Classes are matched by name with a leading `T` of the OMG's
templates removed; members by name, number of parameters and `const`. Reported: members the
OMG declares and ours does not (`MISSING`), a different `const` (`CONST`), a different return
type (`RET`), a parameter taken by value in one and by reference in the other (`SHAPE`) and a
different return by value or by reference (`RETSHAPE`). The comparison is textual: a name
that a macro or an alias spells differently shows up as a difference to be read, not as a
verdict. It does not read exception specifications or default arguments.
"""
import re,sys,os,collections
def strip(src):
    src=re.sub(r'/\*.*?\*/','',src,flags=re.S)
    src=re.sub(r'//[^\n]*','',src)
    src=re.sub(r'^\s*#[^\n]*(\\\n[^\n]*)*','',src,flags=re.M)
    return src
def classes(src):
    out=[]
    for m in re.finditer(r'\b(class|struct)\s+([A-Za-z_]\w*)\s*(?:<[^{;]*>)?\s*(?::[^{;]*)?\{',src):
        i=m.end(); depth=1
        j=i
        while j<len(src) and depth:
            c=src[j]
            if c=='{': depth+=1
            elif c=='}': depth-=1
            j+=1
        out.append((m.group(2),src[i:j-1]))
    return out
def members(body):
    # remove nested braces content (inline bodies)
    res=[];depth=0;cur=''
    for c in body:
        if c=='{':
            if depth==0:
                pass
                res.append(cur); cur=''
            depth+=1
        elif c=='}':
            depth-=1
        elif depth==0:
            if c==';':
                res.append(cur); cur=''
            else: cur+=c
    return [r.strip() for r in res if r.strip()]
def norm(t):
    t=re.sub(r'\s+','',t)
    t=re.sub(r'(?<![\w:])(?:::)?(?:(?!std::)[a-z_]\w*::)+','',t)
    t=t.replace('::','::')
    t=re.sub(r'^::','',t)
    return t
def sig(decl):
    d=re.sub(r'\b(public|private|protected)\s*:','',decl).strip()
    d=re.sub(r'^template\s*<.*?>\s*','',d,flags=re.S)
    if '(' not in d or d.startswith(('typedef','using','friend','OMG_','static_assert')): return None
    d=re.sub(r'\b(virtual|inline|explicit|constexpr|OMG_\w+)\b','',d)
    m=re.match(r'(.*?)\b(~?[A-Za-z_]\w*|operator\s*[^\s(]+)\s*\((.*)\)\s*(const)?\s*(noexcept)?\s*(=\s*(default|delete|0))?\s*$',d,flags=re.S)
    if not m: return None
    ret,name,params,const,_,_,_=m.groups()
    # params: split at top-level commas, drop names
    ps=[];depth=0;cur=''
    for c in params:
        if c in '<([': depth+=1
        if c in '>)]': depth-=1
        if c==',' and depth==0: ps.append(cur);cur=''
        else: cur+=c
    if cur.strip(): ps.append(cur)
    out=[]
    for p in ps:
        p=re.sub(r'=.*$','',p.strip(),flags=re.S).strip()
        toks=p.split()
        if len(toks)>=2 and re.match(r'^[A-Za-z_]\w*$',toks[-1]) and toks[-1] not in ('int','char','long','short','double','float','bool','unsigned','void','const'):
            if not p.endswith(('&','*')) and not re.match(r'.*[<>:]$',p): p=' '.join(toks[:-1])
        else:
            m2=re.match(r'(.*[&*])\s*[A-Za-z_]\w*$',p)
            if m2: p=m2.group(1)
        out.append(norm(p))
    return (norm(ret) if ret.strip() else '', name.replace(' ',''), tuple(out), bool(const))
def collect(root):
    db=collections.defaultdict(set)
    for dp,_,fs in os.walk(root):
        if '/detail' in dp: continue
        for f in fs:
            if not f.endswith('.hpp'): continue
            src=strip(open(os.path.join(dp,f),errors='ignore').read())
            for cname,body in classes(src):
                for d in members(body):
                    s=sig(d)
                    if s: db[cname].add(s)
    return db
omg=collect(sys.argv[1])
import subprocess,tempfile
_root=os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
_tu=tempfile.NamedTemporaryFile('w',suffix='.cpp',delete=False)
_tu.write('#include <dds/dds.hpp>\n#include <dds/core/xtypes/TypeProvider.hpp>\n#include <dds/topic/MultiTopic.hpp>\n'); _tu.close()
_pre=_tu.name+'.pre'
subprocess.run([os.environ.get('CXX','clang++'),'-std=c++17','-E','-P','-I'+_root+'/crates/cpp/include','-I'+_root+'/crates/zerodds-c-api/include','-I'+_root+'/runtime/cpp/include',_tu.name,'-o',_pre],check=True)
sys.argv=[sys.argv[0],sys.argv[1],_pre]+sys.argv[2:]
def collect_file(path):
    db=collections.defaultdict(set)
    src=strip(open(path,errors='ignore').read())
    for cname,body in classes(src):
        for d in members(body):
            s=sig(d)
            if s: db[cname].add(s)
    return db
ours=collect_file(sys.argv[2])
print('classes omg',len(omg),'ours',len(ours))
missing=0;constdiff=[];retdiff=[]
for c,sigs in sorted(omg.items()):
    mine=ours.get(c)
    if mine is None: continue
    by_name=collections.defaultdict(list)
    for s in mine: by_name[(s[1],len(s[2]))].append(s)
    for s in sigs:
        cands=by_name.get((s[1],len(s[2])),[])
        if not cands: 
            constdiff.append((c,'MISSING',s)); continue
        exact=[x for x in cands if x[2]==s[2] and x[3]==s[3] and (x[0]==s[0])]
        if exact: continue
        sameparams=[x for x in cands if x[2]==s[2]]
        if sameparams:
            retdiff.append((c,s,sameparams[0]))
        else:
            constdiff.append((c,'PARAMS',s,cands[0]))
print('missing/params differences',len(constdiff),'return/const differences',len(retdiff))
import json
json.dump({'a':[str(x) for x in constdiff],'b':[str(x) for x in retdiff]},open('/tmp/sigdiff.json','w'),indent=0)
for x in constdiff[:0]: print(x)

print('---- loose comparison')
def key(c): 
    return c[1:] if c.startswith('T') and len(c)>1 and c[1].isupper() else c
def merge(db):
    out=collections.defaultdict(set)
    for c,s in db.items():
        out[key(c)] |= s
    return out
O=merge(omg); M=merge(ours)
def rname(r):
    r=r.replace('const','').replace('&','').replace('*','')
    r=re.sub(r'<.*>','',r)
    return r
missing=[];constd=[];retd=[];classes_missing=[]
for c,sigs in sorted(O.items()):
    if c not in M: classes_missing.append(c); continue
    mine=collections.defaultdict(list)
    for s in M[c]: mine[s[1]].append(s)
    for s in sigs:
        if s[1].startswith('operator') or s[1].startswith('~'): continue
        cands=[x for x in mine.get(s[1],[]) if len(x[2])==len(s[2])]
        if not cands:
            # allow fewer/more default params
            if mine.get(s[1]): continue
            missing.append((c,s)); continue
        if not any(x[3]==s[3] for x in cands):
            constd.append((c,s,cands[0]))
        elif not any(rname(x[0])==rname(s[0]) or s[0] in ('','void') or x[0]=='' for x in cands):
            retd.append((c,s,cands[0]))
print('classes of the OMG without a counterpart:',len(classes_missing))
print(classes_missing)
print('methods missing by name:',len(missing))
for m in missing: print('  MISSING',m[0],m[1][1],m[1][2])
print('const differs:',len(constd))
for m in constd: print('  CONST',m[0],m[1][1],m[1][2],'omg const=',m[1][3],' ours=',m[2][3])
print('return differs:',len(retd))
for m in retd: print('  RET',m[0],m[1][1],'omg',m[1][0],'ours',m[2][0])

print('---- omg detail')
for c in sys.argv[3:]:
    for s in sorted(O.get(c,[])): print(c,s)

print('---- parameter types')
def comparable(skip_unnamed_return=False):
    """(class, OMG signature, our signatures of the same name) for each OMG
    member that is no operator, destructor or constructor."""
    for c,sigs in sorted(O.items()):
        if c not in M: continue
        mine=collections.defaultdict(list)
        for s in M[c]: mine[s[1]].append(s)
        for s in sigs:
            if s[1].startswith('operator') or s[1].startswith('~') or s[1]==c or s[1]=='T'+c: continue
            if skip_unnamed_return and s[0]=='': continue
            yield c,s,mine.get(s[1],[])

def pn(p):
    p=p.replace('const','').replace('&','').replace('*','')
    p=re.sub(r'typename','',p)
    p=p.replace('Type','type')
    return p
pd=[]
for c,s,named in comparable():
    cands=[x for x in named if len(x[2])==len(s[2])]
    if not cands: continue
    if any([pn(a) for a in x[2]]==[pn(a) for a in s[2]] for x in cands): continue
    # pass-by-value vs by-ref differences
    if any(x[2]==s[2] for x in cands): continue
    pd.append((c,s,cands))
print(len(pd))
for c,s,cands in pd: print('  PARAM',c,s[1],'omg',s[2],'ours',[x[2] for x in cands][:2])

print('---- by value / by reference')
def shape(p):
    return ('&' in p, '*' in p)
vd=[]
for c,s,named in comparable():
    cands=[x for x in named if len(x[2])==len(s[2]) and [pn(a) for a in x[2]]==[pn(a) for a in s[2]]]
    if not cands: continue
    if any([shape(a) for a in x[2]]==[shape(a) for a in s[2]] for x in cands): continue
    vd.append((c,s,cands))
print(len(vd))
for c,s,cands in vd: print('  SHAPE',c,s[1],'omg',s[2],'ours',[x[2] for x in cands][:2])

print('---- return shape')
def rn(r):
    r=r.replace('static','').replace('typename','').replace('Type','type').replace('constexpr','')
    return r
rd=[]
for c,s,named in comparable(skip_unnamed_return=True):
    cands=[x for x in named if len(x[2])==len(s[2]) and x[3]==s[3]]
    if not cands: continue
    want=rn(s[0]).replace('T'+c,c)
    if any(rn(x[0]).replace('T'+c,c)==want for x in cands): continue
    rd.append((c,s[1],s[0],[x[0] for x in cands]))
print(len(rd))
for r in rd: print('  RETSHAPE',r)
