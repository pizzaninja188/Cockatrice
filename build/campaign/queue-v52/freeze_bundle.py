import hashlib, json, subprocess
from pathlib import Path
ROOT=Path('/home/ubuntu/Cockatrice'); OUT=ROOT/'build/campaign/queue-v52/freeze'; OUT.mkdir(parents=True,exist_ok=True)
def sha(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for chunk in iter(lambda:f.read(1<<20),b''): h.update(chunk)
    return h.hexdigest()
def git(*a): return subprocess.check_output(['git','-C',str(ROOT),*a],text=True)
cards=[l.split('\t') for l in (ROOT/'build/campaign/queue-v52/cards.tsv').read_text().splitlines()[1:]]
paths=[
 'tricerules/tricerules-core/tests/scenario/q52_cohorts_batch.rs',
 'tricerules/tricerules-core/tests/scenario.rs',
 'tricerules/tricerules-cards/tests/q52_cohorts_batch.rs',
 'tricerules/tricerules-cards/tests/suites/mechanic_registry.rs',
 'tricerules/tricerules-cards/presentation/oracle_fingerprints.tsv',
 'tricerules/CARDS.md',
 'build/campaign/queue-v52/build_queue.py','build/campaign/queue-v52/gen_ron.py','build/campaign/queue-v52/gen_maps.py','build/campaign/queue-v52/gen_records.py','build/campaign/queue-v52/freeze_bundle.py','build/campaign/queue-v52/cards.tsv','build/campaign/queue-v52/revalidation.tsv','build/campaign/queue-v52/selection.md','build/campaign/queue-v52/report.md','build/campaign/queue-v52/map-list.json','build/campaign/queue-v52/families/01-destroy-artifact-land/family.md']
for fam,oid,name in cards:
    cid=name.lower().replace(' ','_').replace("'",'')
    paths += [f'tricerules/tricerules-cards/data/{cid}.ron', f'tricerules/tricerules-cards/authoring/review-maps/{cid}.json']
fdir=ROOT/'build/campaign/queue-v52/families/01-destroy-artifact-land'
paths += [str(p.relative_to(ROOT)) for p in sorted((fdir/'source').glob('*.json'))]
paths += [str(p.relative_to(ROOT)) for p in sorted((fdir/'rulings').glob('*.jsonl'))]
paths=sorted(set(paths))
manifest={'format_version':1,'bundle':'batch-d','families':['01-destroy-artifact-land'],'base_head':git('rev-parse','HEAD').strip(),'review_type':'frozen implementation review','test_evidence':{'scenario_green':'build/verification-logs/20261121-q52-scenario-green.log','registry_green':'build/verification-logs/20261121-q52-registry-green.log','intended_red':['build/verification-logs/20261121-q52-intended-red.log'],'evidence_preparation':'build/verification-logs/card-evidence-d112cdb7-eb59-4b4a-9f95-7a5a09753d08','metadata_refresh':'build/verification-logs/card-data-9743fa9f-0d12-414d-b7c1-76ea359d8033'},'files':[],'dependencies':{},'artifacts':{}}
patch=[]
for rel in paths:
    p=ROOT/rel; assert p.is_file(), rel; d=sha(p)
    manifest['files'].append({'path':rel,'sha256':d,'bytes':p.stat().st_size}); manifest['dependencies'][rel]=d
    if rel in git('ls-files','--',rel).splitlines(): patch.append(git('diff','--',rel))
    else:
        r=subprocess.run(['git','diff','--no-index','--','/dev/null',rel],cwd=ROOT,text=True,capture_output=True); patch.append(r.stdout)
pp=OUT/'batch-d.patch.diff'; pp.write_text(''.join(patch),encoding='utf-8')
manifest['artifacts'][pp.name]=sha(pp)
(OUT/'batch-d.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print('batch-d:',len(paths),'files, patch',sha(pp)[:12])
