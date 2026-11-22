import json
from pathlib import Path
ROOT=Path('/home/ubuntu/Cockatrice'); MAPS=ROOT/'tricerules/tricerules-cards/authoring/review-maps'; OUT=ROOT/'build/campaign/queue-v52'
cards=[l.split('\t') for l in (OUT/'cards.tsv').read_text().splitlines()[1:]]
effects={
 'demolish':('Destroy','/faces/0/spell_effect/0',False,False),
 'sunder_from_within':('Destroy','/faces/0/spell_effect/0',False,False),
 'pillage':('DestroyPreventingRegeneration','/faces/0/spell_effect/0',False,False),
 'victorious_destruction':('Destroy','/faces/0/spell_effect/0',True,False),
 'violent_impact':('Destroy','/faces/0/spell_effect/0',False,True),
}
for fam,oid,name in cards:
    cid=name.lower().replace(' ','_')
    symbol,path,drains,cycling=effects[cid]
    refs=[('SpellEffectKind::'+symbol,path)]
    spans=[(1,1,['/faces/0/spell_effect/0'])]
    if drains:
        refs.append(('SpellEffectKind::LoseLife','/faces/0/spell_effect/1'))
        spans[0]=(1,1,['/faces/0/spell_effect/0','/faces/0/spell_effect/1'])
    if cycling:
        refs += [
          ('SpellEffectKind::Draw','/faces/0/activated_abilities/0/effect/0'),
          ('AbilitySourceZone::Hand','/faces/0/activated_abilities/0/source_zone'),
          ('AbilityCost::Mana','/faces/0/activated_abilities/0/costs/0'),
          ('AbilityCost::DiscardSelf','/faces/0/activated_abilities/0/costs/1'),
        ]
        spans.append((2,2,['/faces/0/activated_abilities/0']))
    payload={
      'format_version':1,'oracle_id':oid,
      'spans':[{'face_id':cid,'start_line':s,'end_line':e,'typed_paths':paths} for s,e,paths in spans],
      'primitive_references':[{'symbol':sym,'typed_path':p} for sym,p in refs],
      'tokens':[],
      'semantic_fixtures':[
        {'test':'scenario q52_cohorts_batch::q52_canary_simplest_demolish_destroys_an_artifact','covers':'actual Demolish destroys an artifact'},
        {'test':'scenario q52_cohorts_batch::q52_canary_hardest_victorious_destruction_destroys_a_land_and_drains','covers':'actual Victorious Destruction destroys a land and drains its controller'},
        {'test':'scenario q52_cohorts_batch::q52_cards_accept_each_branch_and_reject_other_types','covers':'actual card accepts artifact and land and rejects other types'},
        {'test':'scenario q52_cohorts_batch::q52_cards_fizzle_a_departed_target','covers':'actual card fizzles a departed target'},
        {'test':'scenario q52_cohorts_batch::q52_pillage_prevents_regeneration','covers':'actual Pillage ignores a regeneration shield'},
        {'test':'scenario q52_cohorts_batch::q52_violent_impact_cycles_for_two_and_draws','covers':'actual Violent Impact cycles for two and draws one'},
        {'test':'mechanic_registry::q52_cohorts_batch::q52_maps_definitions_and_target_unions','covers':'reviewed identity and typed shape'},
        {'test':'mechanic_registry::q52_cohorts_batch::q52_fingerprints_match_the_pinned_oracle_text','covers':'pinned normalized Oracle text fingerprint'},
      ],
      'complete_definition_review_confirmed':True,
    }
    (MAPS/f'{cid}.json').write_text(json.dumps(payload,indent=2)+'\n',encoding='utf-8')
print('wrote',len(cards),'maps')
