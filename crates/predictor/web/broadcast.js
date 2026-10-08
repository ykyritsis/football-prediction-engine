let replayStage=null;
function simulationControlsHTML(){
 return `<section class="sim-section"><div class="section-head"><div><div class="eyebrow">MATCHDAY STUDIO</div><h3>Make the match happen.</h3></div><span class="studio-badge">SIMULATION</span></div><p class="fine-print">One possible result. A fresh story every time.</p><div class="shape-controls">${['home','away'].map(side=>`<label class="field">${side==='home'?'Home':'Away'} shape<select id="${side}Shape">${['4-3-3','4-2-3-1','4-4-2','3-5-2','3-4-3','4-1-4-1'].map(v=>`<option value="${v}" ${v===state[side+'Shape']?'selected':''}>${v}</option>`).join('')}</select></label>`).join('')}<label class="field">Playback<select id="playback"><option value="5000">Quick · 5 seconds</option><option value="12000">Cinematic · 12 seconds</option><option value="0">Instant result</option></select></label><button class="primary play-button" id="runSimulation"><span>▶</span> Play the match</button></div><details class="simulation-options"><summary>Replay options & starting XIs</summary><label class="seed-option"><input type="checkbox" id="fixedSeed"> Use a repeatable seed</label><label class="field" id="seedField" hidden>Seed<input type="number" id="seed" min="0" max="4294967295" step="1" value="42"></label><div id="simLineups"></div><p class="fine-print">Illustrative starting XIs ranked by available appearances and position. Shape changes alter the displayed XI and contributor allocation, not team outcome probabilities. Upcoming lineups and player availability are unconfirmed.</p></details><div id="simulationOutput"></div></section>`;
}
function simulationXI(m,l){return [m.home,m.away].map((name,side)=>{const t=profile(name);return projectedXI(t,t.player_stats?.[l.key]?.players,[state.homeShape,state.awayShape][side]);});}
function drawBroadcast(canvas,lineups,names,shapes,seed){
 const context=canvas.getContext('2d'),width=960,height=480;canvas.width=1920;canvas.height=960;context.scale(2,2);
 const positions=lineups.map((p,i)=>pitchPositions(p,shapes[i]));
 let frame,minute=0,lastEvent=null,eventTime=0,paused=false,finished=false;
 const teamColors=[color(profile(names[0])),color(profile(names[1]))];
 const rgb=hex=>[1,3,5].map(i=>parseInt(hex.slice(i,i+2),16));
 if(Math.hypot(...rgb(teamColors[0]).map((v,i)=>v-rgb(teamColors[1])[i]))<110)teamColors[1]='#e8eef6';
 const numberColors=teamColors.map(hex=>rgb(hex).reduce((sum,v,i)=>sum+v*[.2126,.7152,.0722][i],0)>155?'#162d32':'#fff');
 const ball={x:480,y:240};const started=performance.now();
 function draw(now){
  const t=paused?(minute*50):(now-started),phase=t/650,elapsed=now-eventTime;
  context.fillStyle='#102d29';context.fillRect(0,0,width,height);
  for(let i=0;i<12;i++){context.fillStyle=i%2?'#164339':'#19483d';context.fillRect(40+i*73,35,73,410);}
  context.strokeStyle='#ffffff62';context.lineWidth=1.6;context.strokeRect(40,35,880,410);context.beginPath();context.moveTo(480,35);context.lineTo(480,445);context.stroke();context.beginPath();context.arc(480,240,60,0,Math.PI*2);context.stroke();context.strokeRect(40,145,130,190);context.strokeRect(790,145,130,190);context.strokeRect(40,195,42,90);context.strokeRect(878,195,42,90);context.fillStyle='#ffffff44';context.fillRect(27,211,13,58);context.fillRect(920,211,13,58);
  const attack=lastEvent?.side??Math.floor(phase/3)%2;
  const targetX=lastEvent&&elapsed<650?(attack===0?922:38):140+(Math.sin(phase*.9)*.5+.5)*680;
  const targetY=lastEvent&&elapsed<650?240+Math.sin(phase)*20:130+(Math.cos(phase*1.7)*.5+.5)*220;
  if(!paused){ball.x+=(targetX-ball.x)*.09;ball.y+=(targetY-ball.y)*.09;}
  context.strokeStyle='#cdebc73b';context.setLineDash([5,8]);context.beginPath();context.moveTo(ball.x-(attack===0?110:-110),ball.y+50);context.lineTo(ball.x,ball.y);context.stroke();context.setLineDash([]);
  positions.forEach((rows,side)=>rows.forEach(({player:p,x,y},index)=>{
   const px=side===0?65+x*370:895-x*370,py=50+y*375;
   const sx=px+Math.sin(phase*.6+index)*8+(side===attack?15:-8),sy=py+Math.cos(phase*.4+index)*9;
   context.fillStyle='#00000030';context.beginPath();context.ellipse(sx,sy+14,14,5,0,0,Math.PI*2);context.fill();
   context.fillStyle=teamColors[side];context.strokeStyle='#ffffffc0';context.lineWidth=1.4;context.beginPath();context.arc(sx,sy,12,0,Math.PI*2);context.fill();context.stroke();
   context.fillStyle=numberColors[side];context.font='bold 10px Arial';context.textAlign='center';context.textBaseline='middle';context.fillText(String(p.number||index+1),sx,sy);
  }));
  context.fillStyle='#00000040';context.beginPath();context.ellipse(ball.x,ball.y+7,7,3,0,0,Math.PI*2);context.fill();context.fillStyle='#fff';context.beginPath();context.arc(ball.x,ball.y,5.5,0,Math.PI*2);context.fill();context.strokeStyle='#163831';context.lineWidth=1.5;context.stroke();
  if(lastEvent?.kind==='goal'&&elapsed<850){context.fillStyle=`rgba(225,191,112,${.13*(1-elapsed/850)})`;context.fillRect(0,0,width,height);const random=seededRandom(seed+lastEvent.minute);for(let i=0;i<35;i++){const angle=random()*Math.PI*2,speed=30+random()*220;context.fillStyle=i%2?'#f0c66a':'#faf4df';const x=480+Math.cos(angle)*speed*elapsed/650,y=240+Math.sin(angle)*speed*elapsed/650+elapsed*elapsed*.00015;context.fillRect(x,y,4,7);}}
  if(!finished)frame=requestAnimationFrame(draw);
 }
 frame=requestAnimationFrame(draw);
 return {update(m){minute=m;},event(e){lastEvent=e;eventTime=performance.now();},pause(v){paused=v;},finish(){finished=true;cancelAnimationFrame(frame);draw(performance.now());},destroy(){finished=true;cancelAnimationFrame(frame);}};
}
function bindSimulation(m,l){
 stopReplay();
 function updateXI(){const xis=simulationXI(m,l);$('simLineups').innerHTML=`<div class="sim-lineup-list">${xis.map((ps,side)=>`<div><strong>${esc(short([m.home,m.away][side]))} · ${esc([state.homeShape,state.awayShape][side])}</strong><p>${ps.map(p=>esc(p.name)).join(' · ')||'Squad data unavailable'}</p></div>`).join('')}</div>`;}
 updateXI();
 for(const side of ['home','away'])$(side+'Shape').onchange=()=>{state[side+'Shape']=$(side+'Shape').value;stopReplay();$('simulationOutput').innerHTML='';updateXI();};
 $('fixedSeed').onchange=()=>{$('seedField').hidden=!$('fixedSeed').checked;};
 $('runSimulation').onclick=()=>{
  stopReplay();
  try{
   let seed;if($('fixedSeed').checked){if(!$('seed').value.trim())throw Error('Enter a seed.');seed=Number($('seed').value);}else{seed=crypto.getRandomValues(new Uint32Array(1))[0];if(seed===previousSeed)seed=(seed+1)>>>0;}previousSeed=seed;
   const xis=simulationXI(m,l),picks=xis.map((ps,side)=>playerForecast(ps,profile([m.home,m.away][side]).player_stats?.[l.key]?.players,m.prediction.scores,side)),match=sampleMatch(m.prediction.scores,picks,seed),story=matchStory(match,picks,seed),aggregate=simulate(m.prediction.scores,10000,seed),names=[m.home,m.away];
   const reduced=matchMedia('(prefers-reduced-motion: reduce)').matches,duration=reduced?0:Number($('playback').value);
   $('simulationOutput').innerHTML=`<div class="broadcast"><div class="broadcast-top"><span class="studio-badge">SIMULATED MATCH</span><span>${esc(l.name)}</span><strong id="replayClock">0′</strong></div><div class="broadcast-scoreboard"><div>${crest(m.home)}<strong>${esc(short(m.home))}</strong><small>${esc(state.homeShape)}</small></div><div class="broadcast-score"><strong id="replayScore">0 – 0</strong><span id="replayPhase">KICK-OFF</span></div><div>${crest(m.away)}<strong>${esc(short(m.away))}</strong><small>${esc(state.awayShape)}</small></div></div><div class="broadcast-pitch"><canvas id="broadcastPitch" aria-label="Illustrative animated football pitch"></canvas><div class="goal-overlay" id="goalOverlay" aria-hidden="true"></div><div class="broadcast-caption" id="pitchCaption">The teams take their positions.</div></div><div class="match-progress"><span id="replayProgress"></span></div><div class="replay-controls"><button class="text-button" id="pauseReplay">Pause</button><button class="text-button" id="skipReplay">Skip to full time →</button><span class="simulated-note">Illustrative events & match stats</span></div><div class="broadcast-bottom"><div><div class="section-head"><h3>Match story</h3><small id="storyCount">Kick-off</small></div><div class="replay-events" id="replayEvents" role="log" aria-live="polite"><div class="replay-event neutral"><span>0′</span><div><strong>Kick-off</strong><small>A new match, a new possibility.</small></div></div></div></div><div class="broadcast-stats"><h3>Simulated match stats</h3><div id="liveMatchStats"></div></div></div><div id="replaySummary"></div></div>`;
   if(!reduced)replayStage=drawBroadcast($('broadcastPitch'),xis,names,[state.homeShape,state.awayShape],seed);
   else {$('broadcastPitch').hidden=true;$('pitchCaption').textContent='Reduced-motion result';}
   let minute=0,index=0,home=0,away=0,paused=false,finished=false;
   const live={shots:[0,0],target:[0,0],saves:[0,0]};
   function liveStats(){const p=minute===0?[50,50]:story.possession;return [['Possession',p.map(v=>v+'%')],['Shots',live.shots],['On target',live.target],['Saves',live.saves]].map(([label,v])=>`<div class="broadcast-stat"><strong>${v[0]}</strong><span>${label}</span><strong>${v[1]}</strong></div>`).join('');}
   function addEvent(e){
    live.shots[e.side]++;if(e.kind==='save'||e.kind==='goal')live.target[e.side]++;if(e.kind==='save')live.saves[1-e.side]++;
    if(e.kind==='goal'){e.side===0?home++:away++;$('replayScore').textContent=`${home} – ${away}`;$('replayScore').classList.remove('goal-pop');void $('replayScore').offsetWidth;$('replayScore').classList.add('goal-pop');
     $('goalOverlay').innerHTML=`<strong>GOAL</strong><span>${esc(e.scorer?.name||short(names[e.side]))}</span>`;$('goalOverlay').classList.remove('celebrate');void $('goalOverlay').offsetWidth;if(duration)$('goalOverlay').classList.add('celebrate');
    }
    const title=e.kind==='goal'?'GOAL':e.kind==='save'?'Saved':'Just wide',text=e.kind==='goal'?`${e.scorer?.name||short(names[e.side])} finds the net.`:e.kind==='save'?`A chance for ${e.scorer?.name||short(names[e.side])}. The keeper holds firm.`:`${e.scorer?.name||short(names[e.side])} goes close.`;
    $('pitchCaption').textContent=text;
    const player=profile(names[e.side]).players.find(p=>p.id===e.scorer?.id),who=player?playerButton(profile(names[e.side]),player,esc(player.name),'text-button'):esc(short(names[e.side]));
    $('replayEvents').insertAdjacentHTML('afterbegin',`<div class="replay-event ${e.kind==='goal'?'goal':'chance'}"><span>${e.minute}′</span><div><strong>${e.kind==='goal'?'⚽ ':''}${title} · ${who}</strong><small>${e.assister?'Assist: '+esc(e.assister.name):esc(short(names[e.side]))}</small></div>${e.kind==='goal'?`<b>${home}–${away}</b>`:''}</div>`);
    replayStage?.event(e);
   }
   function update(target){
    if(!$('replayScore')){stopReplay();return;}minute=Math.min(90,target);replayStage?.update(minute);
    while(index<story.events.length&&story.events[index].minute<=minute)addEvent(story.events[index++]);
    $('replayClock').textContent=minute+'′';$('replayPhase').textContent=minute<45?'FIRST HALF':'SECOND HALF';$('replayProgress').style.width=minute/90*100+'%';$('liveMatchStats').innerHTML=liveStats();$('storyCount').textContent=index+' moments';
    if(minute===90&&!finished){finished=true;clearInterval(replayTimer);replayTimer=null;replayStage?.finish();$('replayPhase').textContent='FULL TIME';$('pitchCaption').textContent=home===away?'Honours even.':short(names[home>away?0:1])+' take the win.';$('pauseReplay').hidden=true;$('skipReplay').hidden=true;
     const contributions=new Map();for(const e of match.events)for(const [p,weight] of [[e.scorer,2],[e.assister,1]])if(p)contributions.set(p.id,{p,side:e.side,score:(contributions.get(p.id)?.score||0)+weight});const star=[...contributions.values()].sort((a,b)=>b.score-a.score)[0],team=star?profile(names[star.side]):null,player=team?.players.find(p=>p.id===star?.p.id);
     $('replaySummary').innerHTML=`<div class="full-time-card"><div><div class="eyebrow">ONE POSSIBLE OUTCOME</div><h3>${home===away?'The points are shared.':esc(short(names[home>away?0:1]))+' win.'}</h3><p>Final score <strong>${home}–${away}</strong> · ${match.events.length} goals · ${story.shots[0]+story.shots[1]} simulated shots</p></div>${player?playerButton(team,player,`${avatar(player)}<span><small>SIMULATION SPOTLIGHT</small><strong>${esc(player.name)}</strong></span>`,'match-star'):''}</div><details class="ensemble"><summary>Explore 10,000 possible outcomes</summary>${probabilities({home_win:aggregate.outcomes[0]/10000,draw:aggregate.outcomes[1]/10000,away_win:aggregate.outcomes[2]/10000})}<p>Average goals: <strong>${aggregate.goals.map(x=>x.toFixed(2)).join(' – ')}</strong>. A sampled score is one outcome, not a prediction guarantee.</p><small>Replay seed ${seed}</small></details><p class="fine-print">Score sampled from the team model. Shot counts, possession, event timings and contributions are synthetic and are not outputs of a trained tactical or player model.</p>`;
     $('runSimulation').innerHTML='<span>↻</span> Play another match';
    }
   }
   $('liveMatchStats').innerHTML=liveStats();
   $('pauseReplay').onclick=()=>{paused=!paused;replayStage?.pause(paused);$('pauseReplay').textContent=paused?'Resume':'Pause';};
   $('skipReplay').onclick=()=>update(90);
   if(duration===0)update(90);else replayTimer=setInterval(()=>{if(!paused)update(minute+1);},duration/90);
   document.querySelector('.broadcast')?.scrollIntoView({behavior:reduced?'instant':'smooth',block:'nearest'});
  }catch(e){$('simulationOutput').textContent=e.message;}
 };
}
