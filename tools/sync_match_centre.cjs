#!/usr/bin/env node
// Extend the saved catalog without discarding existing identities or licensed art.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const ROOT=path.resolve(__dirname,'..'), DIR=path.join(ROOT,'data/catalog'), CACHE=path.join(ROOT,'target/match-centre-cache');
const registry=require('../data/catalog/competitions.json');
const YEAR=Number(process.argv.find(x=>x.startsWith('--year='))?.split('=')[1]||2026);
const AS_OF=process.argv.find(x=>x.startsWith('--as-of='))?.split('=')[1]||`${YEAR}-10-08`;
const REFRESH=process.argv.includes('--refresh');
fs.mkdirSync(CACHE,{recursive:true});
const catalog=JSON.parse(fs.readFileSync(path.join(DIR,'catalog.json')));
const errors=[], sources=[];
const {normalize}=require('./sync_catalog.cjs');
async function get(url,text=false){
 const file=path.join(CACHE,crypto.createHash('sha256').update(url).digest('hex')+(text?'.txt':'.json'));
 if(!REFRESH&&fs.existsSync(file))return text?fs.readFileSync(file,'utf8'):JSON.parse(fs.readFileSync(file));
 const r=await fetch(url,{headers:{'User-Agent':'TouchlineFootball/0.4'},signal:AbortSignal.timeout(15000)});
 if(!r.ok)throw Error(`${r.status} ${url}`);
 const body=await r.text(); const d=text?body:JSON.parse(body); fs.writeFileSync(file,body);return d;
}
async function pool(items,fn,n=4){let i=0;await Promise.all(Array.from({length:n},async()=>{while(i<items.length){const x=items[i++];try{await fn(x)}catch(e){errors.push({item:String(x.key||x.id||x),error:e.message})}}}));}
const feed=(slug,p)=>`https://site.api.espn.com/apis/site/v2/sports/soccer/${slug}/${p}`;
const byId=new Map(Object.values(catalog.teams).map(t=>[t.id,t]));
function portrait(a){return a.headshot?.href?{url:a.headshot.href,credit:'ESPN player photography',license:'Provider image · rights retained by source',source_url:a.links?.find(x=>x.rel?.includes('playercard'))?.href||'https://www.espn.com/soccer/',provider:'ESPN'}:null;}
function stats(a){return a.statistics?.splits?Object.fromEntries(a.statistics.splits.categories.flatMap(c=>c.stats).map(s=>[s.name,s.value])):null;}
function player(a){return {id:String(a.id),name:a.displayName,number:a.jersey||null,age:a.age||null,position:a.position?.displayName||null,position_code:a.position?.abbreviation||null,nationality:a.citizenship||null,height_cm:a.height?Math.round(a.height*2.54):null,weight_kg:a.weight?Math.round(a.weight*.453592):null,birth_date:a.dateOfBirth?.slice(0,10)||null,source_url:a.links?.find(x=>x.rel?.includes('playercard'))?.href||null,stats:stats(a),...(portrait(a)?{portrait:portrait(a)}:{})};}
function register(raw,c){
 let t=byId.get(String(raw.id));
 if(!t){t={id:String(raw.id),name:raw.displayName,display_name:raw.displayName,short_name:raw.shortDisplayName||raw.displayName,abbreviation:raw.abbreviation,color:raw.color,logo:raw.logos?.[0]?.href||raw.logo||null,slug:c.slug,country:c.country,players:[],player_stats:{},competitions:[],kind:c.kind,source_url:raw.links?.find(x=>x.rel?.includes('clubhouse'))?.href||null};byId.set(t.id,t);catalog.teams[t.name]=t;}
 if(!t.competitions.includes(c.key))t.competitions.push(c.key);
 if(c.kind==='international')t.kind='international';
 return t;
}
const eventIndex=new Map();
function importEvent(e,c,files){
 const g=e.competitions?.[0],h=g?.competitors?.find(x=>x.homeAway==='home'),a=g?.competitors?.find(x=>x.homeAway==='away');
 if(!h||!a||!(g.date||e.date))return;
 const status=g.status?.type||e.status?.type; const completed=status?.completed;
 if(completed&&status.name!=='STATUS_FULL_TIME')return;
 if(!completed&&!['STATUS_SCHEDULED','STATUS_POSTPONED'].includes(status?.name))return;
 const date=(g.date||e.date).slice(0,10),sy=Number(date.slice(0,4))-(date.slice(5,7)<'07'?1:0),season=`${sy}-${String((sy+1)%100).padStart(2,'0')}`;
 const ht=byId.get(h.id),at=byId.get(a.id),home=ht?.name||`ESPN:${h.id}:${h.team.displayName}`,away=at?.name||`ESPN:${a.id}:${a.team.displayName}`;
 const gh=Number(h.score),ga=Number(a.score);
 if(completed&&(!Number.isInteger(gh)||!Number.isInteger(ga)||gh<0||ga<0||gh>30||ga>30))return;
 if(files){if(!files.has(season))files.set(season,new Map());files.get(season).set(e.id,{date,team1:home,team2:away,neutral:!!g.neutralSite,...(completed?{score:{ft:[gh,ga]}}:{})});}
 const source_url=e.links?.find(x=>x.rel?.includes('summary'))?.href||null;
 catalog.fixtures[`${c.key}|${date}|${home}|${away}`]={kickoff:g.date||e.date,venue:g.venue?.fullName||null,event_id:e.id,stage:g.altGameNote||null,status:status?.description,neutral:!!g.neutralSite,source_url};
 if(completed&&date<AS_OF)for(const t of [ht,at])if(t){if(!eventIndex.has(t.id))eventIndex.set(t.id,new Map());eventIndex.get(t.id).set(e.id,{id:e.id,date,slug:c.slug,key:c.key});}
}
function save(){catalog.competitions=registry.map(c=>({...c,...catalog.competitions.find(x=>x.key===c.key)}));catalog.expansion_updated_at=new Date().toISOString();catalog.expansion_errors=errors;fs.writeFileSync(path.join(DIR,'catalog.json'),JSON.stringify(catalog,null,2)+'\n');}
async function main(){
 const oldKeys=new Set(catalog.competitions.map(c=>c.key));
 for(const c of registry.filter(c=>!oldKeys.has(c.key)&&!c.provider)){
  console.log('Adding',c.name); const d=await get(feed(c.slug,'teams?limit=200'));const list=d.sports?.[0]?.leagues?.[0]?.teams;
  if(!list?.length){errors.push({competition:c.key,error:'No team registry supplied'});continue;}
  const board=await get(feed(c.slug,'scoreboard'));catalog.competitions.push({...c,logo:board.leagues?.[0]?.logos?.[0]?.href||null,team_ids:list.map(x=>x.team.id)});
  for(const x of list)register(x.team,c);
 }
 // Refresh roster detail for new teams; reuse existing raw rosters to add photos.
 console.log('Rosters',byId.size);
 await pool([...byId.values()],async t=>{
  let roster;
  if(oldKeys.has(t.competitions[0])){
   const u=feed(t.slug,`teams/${t.id}/roster`),hash=crypto.createHash('sha256').update(u).digest('hex'),f=path.join(ROOT,'target/feed-cache',hash+'.json');
   roster=fs.existsSync(f)?JSON.parse(fs.readFileSync(f)):await get(u);
  }else roster=await get(feed(t.slug,`teams/${t.id}/roster`));
  const previous=new Map((t.players||[]).map(p=>[p.id,p]));
  t.players=(roster.athletes||[]).map(a=>{const p=player(a),old=previous.get(p.id);return {...old,...p,portrait:old?.portrait||p.portrait};});
  t.season=roster.season?.displayName||t.season;t.profile_updated_at=roster.timestamp||new Date().toISOString();t.player_stats||={};
  for(const key of t.competitions){const c=registry.find(c=>c.key===key);let r=c.slug===t.slug?roster:await get(feed(c.slug,`teams/${t.id}/roster`));t.player_stats[key]={season:r.season?.displayName||null,players:Object.fromEntries((r.athletes||[]).map(a=>[String(a.id),stats(a)]))};}
 });
 save();
 // New competition histories use bounded yearly requests. Existing domestic
 // histories remain untouched; their scoreboards supply dated lineup candidates.
 for(const c of registry.filter(c=>!c.provider)){
  const newHistory=!oldKeys.has(c.key);const files=new Map();let failed=false;
  const years=newHistory?Array.from({length:YEAR-2018+2},(_,i)=>2018+i):[YEAR];
  console.log('Match archive',c.name,years.length,'years');
  await pool(years,async year=>{try{const u=feed(c.slug,`scoreboard?dates=${year}&limit=1000`),d=await get(u);if(!Array.isArray(d.events)||d.events.length>=1000)throw Error('Missing or truncated event list');for(const e of d.events)importEvent(e,c,newHistory?files:null);sources.push({competition:c.key,url:u,fetched_at:new Date().toISOString(),events:d.events.length});}catch(e){failed=true;throw e;}},3);
  if(newHistory&&!failed)for(const [season,m] of files){const dir=path.join(ROOT,'data/results',season);fs.mkdirSync(dir,{recursive:true});fs.writeFileSync(path.join(dir,c.key+'.json'),JSON.stringify({name:c.name,source:'ESPN',matches:[...m.values()].sort((a,b)=>a.date.localeCompare(b.date))},null,2)+'\n');}
  save();
 }
 // Three recent match sheets per team provide actual lineups and player game logs.
 const events=new Map();
 for(const xs of eventIndex.values())for(const e of [...xs.values()].sort((a,b)=>b.date.localeCompare(a.date)).slice(0,3))events.set(e.id,e);
 console.log('Dated match sheets',events.size);
 await pool([...events.values()],async e=>{
  const d=await get(feed(e.slug,`summary?event=${e.id}`));
  const game=d.header?.competitions?.[0],teams=game?.competitors||[];
  for(const r of d.rosters||[]){const t=byId.get(r.team.id);if(!t)continue;
   const lineup={date:e.date,event_id:e.id,competition:e.key,formation:r.formation||null,source_url:teams.length?`https://www.espn.com/soccer/match/_/gameId/${e.id}`:null,players:r.roster.filter(a=>a.starter).map(a=>({id:String(a.athlete.id),name:a.athlete.displayName,number:a.jersey,position_code:a.position?.abbreviation,place:Number(a.formationPlace)||null}))};
   if(!t.last_lineup||lineup.date>t.last_lineup.date)t.last_lineup=lineup;
   const opponent=teams.find(x=>x.id!==t.id)?.team?.displayName;
   for(const a of r.roster){const p=t.players?.find(p=>p.id===a.athlete.id);if(!p)continue;if(!p.portrait&&portrait(a.athlete))p.portrait=portrait(a.athlete);p.recent_matches||=[];if(!p.recent_matches.some(m=>m.event_id===e.id))p.recent_matches.push({date:e.date,event_id:e.id,competition:e.key,opponent,starter:!!a.starter,stats:Object.fromEntries((a.stats||[]).map(x=>[x.name,x.value])),source_url:lineup.source_url});p.recent_matches.sort((a,b)=>b.date.localeCompare(a.date));p.recent_matches=p.recent_matches.slice(0,5);}
  }
 },3);
 // Reuse checked identities across club/national rosters for portrait coverage.
 const pictures=new Map();for(const t of byId.values())for(const p of t.players||[])if(p.portrait&&p.birth_date)pictures.set(p.id+'|'+p.birth_date,p.portrait);
 for(const t of byId.values())for(const p of t.players||[])if(!p.portrait&&pictures.has(p.id+'|'+p.birth_date))p.portrait=pictures.get(p.id+'|'+p.birth_date);
 save();fs.writeFileSync(path.join(DIR,'expansion-sources.json'),JSON.stringify({as_of:AS_OF,sources,errors},null,2)+'\n');
 console.log(JSON.stringify({teams:byId.size,players:[...byId.values()].reduce((n,t)=>n+(t.players?.length||0),0),portraits:[...byId.values()].reduce((n,t)=>n+(t.players?.filter(p=>p.portrait).length||0),0),lineups:[...byId.values()].filter(t=>t.last_lineup).length,errors},null,2));
 if(errors.length)process.exitCode=1;
}
module.exports={portrait,player,importEvent};
if(require.main===module)main().catch(e=>{save();console.error(e);process.exitCode=1;});
