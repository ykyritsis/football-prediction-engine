#!/usr/bin/env node
// Rehydrate dated formation records from saved match sheets without refreshing squads or art.
const fs=require('node:fs'),path=require('node:path');
const {usualFormation}=require('../crates/predictor/web/presentation.js');
const root=path.resolve(__dirname,'..'),file=path.join(root,'data/catalog/catalog.json');
const catalog=JSON.parse(fs.readFileSync(file)),registry=require('../data/catalog/competitions.json');
const asOf=process.argv.find(a=>a.startsWith('--as-of='))?.split('=')[1]||'2026-10-08';
const teams=new Map(Object.values(catalog.teams).map(t=>[String(t.id),t]));
const cache=path.join(root,'target/match-centre-cache');
let sheets=0;
for(const name of fs.readdirSync(cache)){
 if(!name.endsWith('.json'))continue;const d=JSON.parse(fs.readFileSync(path.join(cache,name)));
 const match=d.header?.competitions?.[0],date=match?.date?.slice(0,10);
 if(!d.rosters||!date||date>asOf||!match.status?.type?.completed)continue;
 const competition=registry.find(c=>c.slug===d.header.league?.slug)?.key;
 if(!competition)continue;
 for(const r of d.rosters){
  const t=teams.get(String(r.team.id));if(!t)continue;
  const sheet={date,event_id:String(d.header.id),competition,formation:r.formation||null,source_url:`https://www.espn.com/soccer/match/_/gameId/${d.header.id}`,players:(r.roster||[]).filter(p=>p.starter).map(p=>({id:String(p.athlete.id),name:p.athlete.displayName,number:p.jersey,position_code:p.position?.abbreviation,place:Number(p.formationPlace)||null}))};
  if(sheet.players.length!==11)continue;
  t.lineup_history=[...(t.lineup_history||[]).filter(s=>s.event_id!==sheet.event_id),sheet];sheets++;
 }
}
for(const t of teams.values()){
 t.lineup_history=(t.lineup_history||[]).sort((a,b)=>b.date.localeCompare(a.date)).slice(0,10);
 if(t.lineup_history.length)t.last_lineup=t.lineup_history[0];
 t.formation_profile=usualFormation(t,asOf);
}
fs.writeFileSync(file,JSON.stringify(catalog)+'\n');
console.log(JSON.stringify({sheets,teams_with_recent_formations:[...teams.values()].filter(t=>t.formation_profile.sample>0).length,total_teams:teams.size}));
