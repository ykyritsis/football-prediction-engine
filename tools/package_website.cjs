const fs=require('node:fs'),path=require('node:path');
function packageWebsite(output){
 const htmlFile=path.join(output,'index.html');let html=fs.readFileSync(htmlFile,'utf8');
 const marker=/<script id="data" type="application\/json">\s*([\s\S]*?)\s*<\/script>/;
 const data=JSON.parse(html.match(marker)[1]),assets=path.join(output,'assets');fs.mkdirSync(assets,{recursive:true});
 const compact=(key,value)=>typeof value==='number'&&!Number.isInteger(value)?Math.round(value*1e9)/1e9:value;
 for(const league of data.leagues){const file=`competition-${league.key}.json`;fs.writeFileSync(path.join(assets,file),JSON.stringify({fixtures:league.fixtures,forecasts:league.forecasts,historical:league.historical,archive:league.archive},compact));league.forecasts=[];league.historical=[];league.archive=[];for(const m of league.fixtures){const [h,a]=(m.prediction.predicted_score||m.prediction.most_likely_score);m.prediction.most_likely_probability=m.prediction.scores[h][a];delete m.prediction.scores;}league.lazy=true;league.asset='assets/'+file;}
 for(const t of Object.values(data.catalog.teams)){
  const file=`team-${t.id}.json`,detail={players:t.players,player_stats:t.player_stats,last_lineup:t.last_lineup,lineup_history:t.lineup_history,heritage:data.transfermarkt?.clubs?.[t.name]};
  fs.writeFileSync(path.join(assets,file),JSON.stringify(detail,compact));t.players_count=t.players?.length||0;t.players=[];t.player_stats={};delete t.last_lineup;delete t.lineup_history;t.lazy=true;t.asset='assets/'+file;
 }
 if(data.transfermarkt)data.transfermarkt.clubs={};
 const upcoming=new Set(data.leagues.flatMap(l=>l.fixtures.map(m=>[l.key,m.date,m.home,m.away].join('|'))));
 data.catalog.fixtures=Object.fromEntries(Object.entries(data.catalog.fixtures||{}).filter(([key])=>upcoming.has(key)));
 const safe=JSON.stringify(data,compact).replaceAll('<','\\u003c').replaceAll('>','\\u003e').replaceAll('&','\\u0026');
 html=html.replace(marker,()=>`<script id="data" type="application/json">${safe}</script>`);fs.writeFileSync(htmlFile,html);
 return {initial_bytes:Buffer.byteLength(html),assets:fs.readdirSync(assets).length};
}
module.exports={packageWebsite};
