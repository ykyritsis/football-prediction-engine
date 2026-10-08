// Pure presentation calculations, shared by the browser and Node checks.
function zonedInstant(day,time,zone){
 const [y,m,d]=day.split('-').map(Number),[h,min]=time.split(':').map(Number);
 const target=Date.UTC(y,m-1,d,h,min);let epoch=target;
 const f=new Intl.DateTimeFormat('en-US',{timeZone:zone,year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',second:'2-digit',hourCycle:'h23'});
 for(let i=0;i<3;i++){const p=Object.fromEntries(f.formatToParts(epoch).filter(p=>p.type!=='literal').map(p=>[p.type,p.value]));const seen=Date.UTC(+p.year,+p.month-1,+p.day,+p.hour,+p.minute,+p.second);epoch+=target-seen;}
 return epoch;
}
function kickoffInfo(meta={},day,zone=Intl.DateTimeFormat().resolvedOptions().timeZone){
 let epoch=meta.kickoff?Date.parse(meta.kickoff):NaN;
 if(!Number.isFinite(epoch)&&meta.local_date&&meta.local_time&&meta.time_zone)epoch=zonedInstant(meta.local_date,meta.local_time,meta.time_zone);
 if(!Number.isFinite(epoch))return {date:day,time:'Time TBC',zone,epoch:null,label:day};
 const parts=Object.fromEntries(new Intl.DateTimeFormat('en-US',{timeZone:zone,year:'numeric',month:'2-digit',day:'2-digit'}).formatToParts(epoch).map(p=>[p.type,p.value]));
 return {date:`${parts.year}-${parts.month}-${parts.day}`,time:new Intl.DateTimeFormat('en-US',{timeZone:zone,hour:'numeric',minute:'2-digit',hour12:true}).format(epoch),zone,epoch,label:new Intl.DateTimeFormat('en-US',{timeZone:zone,weekday:'short',month:'short',day:'numeric',hour:'numeric',minute:'2-digit',hour12:true,timeZoneName:'short'}).format(epoch)};
}
function formationRows(shape){const rows=String(shape||'4-3-3').split('-').map(Number);return rows.length>=2&&rows.every(n=>Number.isInteger(n)&&n>0)&&rows.reduce((a,b)=>a+b,0)===10?[1,...rows]:[1,4,3,3];}
function projectedXI(team,stats={},shape='4-3-3'){
 const players=[...(team.players||[])].sort((a,b)=>(stats[b.id]?.appearances||0)-(stats[a.id]?.appearances||0)||String(a.id).localeCompare(String(b.id)));
 const rows=formationRows(shape),chosen=[],used=new Set();
 function take(code,count){let candidates=players.filter(p=>p.position_code===code&&!used.has(p.id));for(const p of [...candidates,...players.filter(p=>!used.has(p.id)&&p.position_code!=='G')]){if(count===0)break;if(used.has(p.id))continue;used.add(p.id);chosen.push(p);count--;}}
 take('G',1);take('D',rows[1]);take('M',rows.slice(2,-1).reduce((a,b)=>a+b,0));take('F',rows.at(-1));
 return chosen.slice(0,11);
}
function pitchPositions(players,shape='4-3-3'){
 const rows=formationRows(shape);let i=0;return rows.flatMap((count,row)=>Array.from({length:count},(_,col)=>({player:players[i++],x:row/(rows.length-1),y:(col+1)/(count+1)}))).filter(x=>x.player);
}
function recordedXI(sheet,squad=[]){
 const players=(sheet.players||[]).map(p=>({...p,...squad.find(x=>x.id===p.id),match_position_code:p.position_code}));
 const rank=p=>{const code=p.match_position_code||'';return code==='G'?0:/^(CD|CB|D)(-|$)/.test(code)?1:/^(LB|RB|LWB|RWB)$/.test(code)?2:/^(DM|CM|LM|RM|M)(-|$)/.test(code)?3:/^AM/.test(code)?4:5;};
 const side=p=>/^(L|.*-L$)/.test(p.match_position_code||'')?0:/^(R|.*-R$)/.test(p.match_position_code||'')?2:1;
 const ordered=players.sort((a,b)=>rank(a)-rank(b)||(a.place||99)-(b.place||99));let index=0;
 return formationRows(sheet.formation).flatMap(count=>{const row=ordered.slice(index,index+count);index+=count;return row.sort((a,b)=>side(a)-side(b));});
}
function validFormation(shape){const rows=String(shape||'').split('-').map(Number);return rows.length>=2&&rows.every(n=>Number.isInteger(n)&&n>0)&&rows.reduce((a,b)=>a+b,0)===10;}
function usualFormation(team,asOf){
 const seen=new Set(),cutoff=asOf?Date.parse(asOf+'T23:59:59Z'):Infinity;
 const sheets=[...(team.lineup_history||[]),...(team.last_lineup?[team.last_lineup]:[])].filter(s=>{
  const id=s.event_id||s.date,day=Date.parse(s.date);if(seen.has(id)||!validFormation(s.formation)||!Number.isFinite(day)||day>cutoff)return false;seen.add(id);return true;
 }).sort((a,b)=>b.date.localeCompare(a.date));
 const recent=sheets.filter(s=>!asOf||cutoff-Date.parse(s.date)<=180*86400000).slice(0,5);
 if(!recent.length)return {shape:sheets[0]?.formation||'4-3-3',sample:0,used:0,latest:sheets[0]?.date||null,counts:[],basis:sheets.length?'Last recorded shape':'Suggested shape'};
 const counts=new Map();for(const s of recent){const row=counts.get(s.formation)||{shape:s.formation,count:0,date:s.date};row.count++;counts.set(s.formation,row);}
 const ranked=[...counts.values()].sort((a,b)=>b.count-a.count||b.date.localeCompare(a.date));
 return {shape:ranked[0].shape,sample:recent.length,used:ranked[0].count,latest:recent[0].date,counts:ranked,basis:recent.length===1?'Last recorded shape':'Usual shape'};
}
function selectedFormation(team,overrides,asOf){return overrides.get(String(team.id||team.name))||usualFormation(team,asOf).shape;}
// One monotonic clock drives both the replay timeline and pitch. Pauses preserve elapsed time.
function replayClock(duration){
 let elapsed=0,last=null,paused=false;
 return {tick(now){if(last!==null&&!paused)elapsed=Math.min(duration,elapsed+Math.max(0,now-last));last=now;return duration?elapsed/duration:1;},pause(value,now){this.tick(now);paused=value;},get elapsed(){return elapsed;},get paused(){return paused;}};
}
function motionBlend(delta,settle=170){return 1-Math.exp(-Math.max(0,delta)/settle);}
if(typeof module!=='undefined')module.exports={zonedInstant,kickoffInfo,formationRows,projectedXI,pitchPositions,recordedXI,validFormation,usualFormation,selectedFormation,replayClock,motionBlend};
