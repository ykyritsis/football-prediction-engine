#!/usr/bin/env node
// National profiles require a unique senior-team name plus a national confederation.
const fs=require('node:fs'),path=require('node:path'),tm=require('./transfermarkt.cjs');
const file=path.resolve(__dirname,'../data/catalog/catalog.json');
const queries={'USA':'United States','South Korea':'Korea Republic','Türkiye':'Turkey','Iran':'Iran','Ivory Coast':"Cote d'Ivoire"};
async function main(){const c=JSON.parse(fs.readFileSync(file)),teams=Object.values(c.teams).filter(t=>t.competitions.includes('world_cup')),issues=[];
 for(const t of teams){try{const q=queries[t.name]||t.name,result=await tm.request('/clubs/search/'+encodeURIComponent(q));const matches=(result.results||[]).filter(x=>[t.name,q].some(n=>tm.normalize(n)===tm.normalize(x.name)));if(matches.length!==1){issues.push({team:t.name,error:'No unique senior national-team match'});continue;}
  const p=await tm.request(`/clubs/${matches[0].id}/profile`);if(!p.confederation){issues.push({team:t.name,error:'Provider did not confirm a national confederation'});continue;}
  tm.snapshot.club_ids[t.name]=matches[0].id;const d=await tm.club(t.name);t.coach=d.profile.coach?.name;t.coach_since=d.profile.coach?.since;fs.writeFileSync(file,JSON.stringify(c)+'\n');console.log('National profile',t.name,t.coach||'manager unavailable');
 }catch(e){issues.push({team:t.name,error:e.message});console.log('National profile unavailable',t.name,e.message);}}
 c.national_profile_issues=issues;fs.writeFileSync(file,JSON.stringify(c)+'\n');console.log(JSON.stringify({requested:teams.length,issues},null,2));}
if(require.main===module)main().catch(e=>{console.error(e);process.exitCode=1;});
