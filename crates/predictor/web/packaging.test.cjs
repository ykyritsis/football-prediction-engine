const {test}=require('node:test'),assert=require('node:assert/strict');
const fs=require('node:fs'),os=require('node:os'),path=require('node:path');
const {packageWebsite}=require('../../../tools/package_website.cjs');
test('public packaging preserves lazy profiles and normalized simulation grids without injecting markup',()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'touchline-package-'));
 try{
  const scores=[[.12345678912,.33333333333],[.33333333333,.20987654422]],p={scores,most_likely_score:[0,1]};
  const match={home:'Club',away:'Away',date:'2026-10-09',prediction:p};
  const data={leagues:[{key:'test_league',fixtures:[match],forecasts:[{prediction:p}],historical:[],archive:[{score:[1,2]}]}],catalog:{teams:{Club:{id:'tm-11',name:'Club',players:[{id:'1',name:'</script> & $&'}],player_stats:{test_league:{players:{1:{totalGoals:3}}}},last_lineup:{formation:'4-3-3'}}},fixtures:{'test_league|2026-10-09|Club|Away':{utc:'2026-10-09T18:00:00Z'},'old':{}}},transfermarkt:{clubs:{Club:{profile:{name:'Club'}}}}};
  fs.writeFileSync(path.join(dir,'index.html'),`<script id="data" type="application/json">${JSON.stringify(data).replaceAll('<','\\u003c')}</script><main>App</main>`);
  packageWebsite(dir);
  const html=fs.readFileSync(path.join(dir,'index.html'),'utf8'),initial=JSON.parse(html.match(/<script id="data" type="application\/json">([\s\S]*?)<\/script>/)[1]);
  assert.equal(initial.leagues[0].fixtures[0].prediction.scores,undefined);
  assert.ok(Math.abs(initial.leagues[0].fixtures[0].prediction.most_likely_probability-1/3)<1e-9);
  assert.deepEqual(initial.catalog.teams.Club.players,[]);
  assert.equal(initial.catalog.teams.Club.players_count,1);
  assert.deepEqual(Object.keys(initial.catalog.fixtures),['test_league|2026-10-09|Club|Away']);
  const team=JSON.parse(fs.readFileSync(path.join(dir,initial.catalog.teams.Club.asset)));
  assert.equal(team.players[0].name,'</script> & $&');assert.equal(team.heritage.profile.name,'Club');assert.equal(team.last_lineup.formation,'4-3-3');
  const competition=JSON.parse(fs.readFileSync(path.join(dir,initial.leagues[0].asset)));
  assert.equal(competition.archive[0].score[1],2);
  assert.ok(Math.abs(competition.fixtures[0].prediction.scores.flat().reduce((a,b)=>a+b,0)-1)<1e-6);
  assert.equal((html.match(/<\/script>/g)||[]).length,1);
 }finally{assert.equal(path.dirname(path.resolve(dir)),path.resolve(os.tmpdir()));assert.ok(path.basename(dir).startsWith('touchline-package-'));fs.rmSync(dir,{recursive:true,force:true});}
});
