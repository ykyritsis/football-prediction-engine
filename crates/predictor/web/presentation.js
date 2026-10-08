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
if(typeof module!=='undefined')module.exports={zonedInstant,kickoffInfo,formationRows,projectedXI,pitchPositions};
