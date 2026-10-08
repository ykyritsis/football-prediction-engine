#!/usr/bin/env node
// Serialized, cached Transfermarkt enrichment. Portraits require verified birth dates.
const fs=require('node:fs'),path=require('node:path');
const tm=require('./transfermarkt.cjs'),ROOT=path.resolve(__dirname,'..'),file=path.join(ROOT,'data/catalog/catalog.json');
const registry=require('../data/catalog/competitions.json'),YEAR=2026;
const norm=tm.normalize;
function save(c){fs.writeFileSync(file,JSON.stringify(c,null,2)+'\n');}
function position(s){return /keeper/i.test(s)?'G':/back|defend/i.test(s)?'D':/forward|winger|striker/i.test(s)?'F':'M';}
const czechIds={'slavia':62,'slaviapraha':62,'sparta':197,'spartapraha':197,'viktoriaplzen':941,'sigmaolomouc':2311,'slovanliberec':697,'banikostrava':377,'hradeckralove':1897,'pardubice':1496,'jablonec':1322,'artisbrno':24325,'zbrojovkabrno':5225,'mladaboleslav':5546,'teplice':814,'slovacko':5544,'bohemians1905':715,'bohemianspraha1905':715,'fastavzlin':5545,'zlin':5545};
function czechName(name,c){let id=czechIds[norm(name.replace(/^1\.\s*/,''))];if(!id)return name;return Object.keys(tm.snapshot.club_ids).find(n=>String(tm.snapshot.club_ids[n])===String(id)&&c.teams[n])||name;}
function parseCzech(text,season,c){let date=null,time=null;const rows=[],year=Number(season.slice(0,4));const months={Jan:1,Feb:2,Mar:3,Apr:4,May:5,Jun:6,Jul:7,Aug:8,Sep:9,Oct:10,Nov:11,Dec:12};
 for(const line of text.split(/\r?\n/)){
  const d=line.trim().match(/^(?:Mon|Tue|Wed|Thu|Fri|Sat|Sun) ([A-Z][a-z]{2}) (\d{1,2})(?: (\d{4}))?$/);
  if(d){const m=months[d[1]];date=`${d[3]||year+(m<7?1:0)}-${String(m).padStart(2,'0')}-${d[2].padStart(2,'0')}`;time=null;continue;}
  if(!date)continue;
  const t=line.match(/^\s*(\d{1,2}:\d{2})\s+/);let body=line.trim();if(t){time=t[1].padStart(5,'0');body=body.slice(t[1].length).trim();}
  let m=body.match(/^(.+?)\s+v\s+(.+?)\s+(\d+)-(\d+)(?:\s+\([^)]*\))?\s*$/),home,away,ft;
  if(m){home=m[1];away=m[2];ft=[+m[3],+m[4]];}else{m=body.match(/^(.+?)\s+(\d+)-(\d+)(?:\s+\(\d+-\d+\))?\s+(.+?)\s*$/);if(m){home=m[1];away=m[4];ft=[+m[2],+m[3]];}else{m=body.match(/^(.+?)\s+v\s+(.+?)$/);if(m){home=m[1];away=m[2];}}}
  if(!home||/[()]/.test(home+away))continue;home=czechName(home,c);away=czechName(away,c);
  rows.push({date,team1:home,team2:away,...(ft?{score:{ft}}:{})});
  // Source times are Czech civil time. Preserve the zone; browser conversion
  // uses the IANA zone and its daylight-saving offset, never an assumed UTC time.
  if(t)c.fixtures[`chance_liga|${date}|${home}|${away}`]={local_date:date,local_time:time,time_zone:'Europe/Prague',source_url:`https://github.com/openfootball/europe/blob/master/czech-republic/${season}_cz1.txt`};
 }
 return rows;
}
async function main(){
 let c=JSON.parse(fs.readFileSync(file));const list=await tm.request('/competitions/TS1/clubs?season_id=2026');
 for(const raw of list.clubs){let name=Object.keys(tm.snapshot.club_ids).find(n=>String(tm.snapshot.club_ids[n])===raw.id&&c.teams[n]);name||=raw.name;tm.snapshot.club_ids[name]=raw.id;
  let t=c.teams[name];if(!t)t=c.teams[name]={id:'tm-'+raw.id,name,display_name:raw.name,short_name:raw.name,logo:`https://img.a.transfermarkt.technology/wappen/big/${raw.id}.png`,slug:'cze.1',country:'Czechia',kind:'domestic',players:[],player_stats:{},competitions:[],source_url:`https://www.transfermarkt.com/-/startseite/verein/${raw.id}`};if(!t.competitions.includes('chance_liga'))t.competitions.push('chance_liga');
 }
 const comp=c.competitions.find(x=>x.key==='chance_liga');Object.assign(comp,{...registry.find(x=>x.key==='chance_liga'),logo:'https://img.a.transfermarkt.technology/logo/big/ts1.png',team_ids:list.clubs.map(x=>'tm-'+x.id)});save(c);
 fs.writeFileSync(path.join(ROOT,'data/catalog/transfermarkt.json'),JSON.stringify(tm.snapshot,null,2)+'\n');
 for(const season of ['2018-19','2020-21','2023-24','2024-25','2026-27']){const u=`https://raw.githubusercontent.com/openfootball/europe/master/czech-republic/${season}_cz1.txt`;const r=await fetch(u,{signal:AbortSignal.timeout(15000)});if(!r.ok)throw Error('Czech archive unavailable '+season);const matches=parseCzech(await r.text(),season,c);if(matches.length<100)throw Error('Unexpected Czech parse count '+season+' '+matches.length);const dir=path.join(ROOT,'data/results',season);fs.mkdirSync(dir,{recursive:true});fs.writeFileSync(path.join(dir,'chance_liga.json'),JSON.stringify({name:'Chance Liga',source:u,matches},null,2)+'\n');console.log('Czech archive',season,matches.length);}
 save(c);await tm.sync(YEAR);
 const teams=Object.values(c.teams).filter(t=>tm.snapshot.club_ids[t.name]);
 console.log('Enriching verified clubs',teams.length);
 let photos=0,verified=0;const issues=[];
 for(const t of teams){try{
  const id=tm.snapshot.club_ids[t.name],roster=await tm.request(`/clubs/${id}/players`);
  if(t.id.startsWith('tm-')){t.players=roster.players.map(p=>({id:'tm-'+p.id,name:p.name,number:p.shirtNumber,age:p.age,position:p.position,position_code:position(p.position),nationality:p.nationality?.join(', '),height_cm:p.height,birth_date:p.dateOfBirth,source_url:`https://www.transfermarkt.com/-/profil/spieler/${p.id}`}));}
  for(const p of t.players||[]){const found=roster.players.filter(x=>norm(x.name)===norm(p.name)&&p.birth_date&&x.dateOfBirth===p.birth_date);if(found.length!==1)continue;const raw=found[0];verified++;p.transfermarkt={id:raw.id,foot:raw.foot,market_value:raw.marketValue,joined:raw.joinedOn,contract:raw.contract,from:raw.signedFrom,captain:raw.isCaptain,position:raw.position,source_url:`https://www.transfermarkt.com/-/profil/spieler/${raw.id}`,updated_at:roster.updatedAt};if(raw.imageUrl){p.alternative_portrait={url:raw.imageUrl,credit:'Transfermarkt player photography',license:'Provider image · rights retained by source',source_url:p.transfermarkt.source_url,provider:'Transfermarkt'};if(!p.portrait){p.portrait=p.alternative_portrait;photos++;}}}
  const history=await tm.club(t.name);t.coach=history.profile.coach?.name;t.coach_since=history.profile.coach?.since;
  if(history.profile.stadiumName)t.venue={...t.venue,name:history.profile.stadiumName,capacity:history.profile.stadiumSeats};
  save(c);console.log('Club enriched',t.name,'portraits',photos);
 }catch(e){issues.push({club:t.name,error:e.message});console.log('Kept previous club data',t.name,e.message);}}
 // Photos and biographical enrichment can transfer between matching national
 // and club records only after ESPN identity and date of birth both agree.
 const people=new Map();for(const t of teams)for(const p of t.players||[])if(p.birth_date)people.set(p.id+'|'+p.birth_date,p);
 for(const t of Object.values(c.teams))for(const p of t.players||[]){const known=people.get(p.id+'|'+p.birth_date);if(known){p.portrait||=known.portrait;p.alternative_portrait||=known.alternative_portrait;p.transfermarkt||=known.transfermarkt;}}
 c.enrichment_updated_at=new Date().toISOString();c.enrichment_errors=issues;save(c);console.log(JSON.stringify({clubs:teams.length,verified_players:verified,new_portraits:photos,issues},null,2));if(issues.length)process.exitCode=1;
}
module.exports={parseCzech,position};
if(require.main===module)main().catch(e=>{console.error(e);process.exitCode=1;});
