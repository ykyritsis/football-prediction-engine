let replayStage=null;
const formationOverrides=new Map();
function formationHint(t){const f=usualFormation(t,data.as_of);return f.sample>1?`${f.used} of ${f.sample} recent recorded XIs`:f.latest?`Recorded ${dateLabel(f.latest)} ${f.latest.slice(0,4)}`:'No recorded formation · editable suggestion';}
function simulationControlsHTML(m){
 const names=[m.home,m.away];
 for(const [i,side] of ['home','away'].entries())state[side+'Shape']=selectedFormation(profile(names[i]),formationOverrides,data.as_of);
 return `<section class="sim-section"><div class="section-head"><div><div class="eyebrow">MATCHDAY STUDIO</div><h3>Play out the possibilities.</h3></div><span class="studio-badge">SIMULATION</span></div><p class="fine-print">Team shapes are filled from recorded lineups. Change them to try your own XI.</p><div class="shape-controls">${['home','away'].map((side,i)=>{const t=profile(names[i]),f=usualFormation(t,data.as_of),manual=formationOverrides.has(String(t.id||t.name)),options=[...new Set([state[side+'Shape'],f.shape,'4-3-3','4-2-3-1','4-4-2','3-5-2','3-4-3','3-4-2-1','4-1-4-1','4-4-1-1','4-3-1-2','5-3-2','5-4-1'])];return `<div class="shape-field"><label class="field">${esc(short(names[i]))} shape<select id="${side}Shape" aria-label="${esc(short(names[i]))} formation">${options.map(v=>`<option value="${v}" ${v===state[side+'Shape']?'selected':''}>${v}</option>`).join('')}</select></label><div class="shape-provenance" id="${side}ShapeHint"><span>${manual?'Your selection · Usual '+esc(f.shape):esc(f.basis)+' · '+esc(formationHint(t))}</span><button class="text-button" data-reset-shape="${side}" ${manual?'':'hidden'}>Reset</button></div></div>`;}).join('')}<label class="field">Playback<select id="playback"><option value="5000">Quick · 5 seconds</option><option value="12000">Cinematic · 12 seconds</option><option value="0">Instant result</option></select></label><button class="primary play-button" id="runSimulation"><span>▶</span> Play the match</button></div><details class="simulation-options"><summary>Replay options & starting XIs</summary><label class="seed-option"><input type="checkbox" id="fixedSeed"> Use a repeatable seed</label><label class="field" id="seedField" hidden>Seed<input type="number" id="seed" min="0" max="4294967295" step="1" value="42"></label><div id="simLineups"></div><p class="fine-print">Illustrative XIs use a recent recorded selection in this shape when all players are in the saved squad; otherwise they use appearances and positions. Shape changes alter the displayed XI and contributor allocation, not team outcome probabilities. Upcoming lineups and player availability are unconfirmed.</p></details><div id="simulationOutput"></div></section>`;
}
function simulationXI(m,l){return [m.home,m.away].map((name,side)=>{const t=profile(name);return projectedXI(t,t.player_stats?.[l.key]?.players,[state.homeShape,state.awayShape][side],data.as_of);});}
function drawBroadcast(canvas,lineups,names,shapes,seed){
 const context=canvas.getContext('2d'),width=960,height=480,ratio=Math.min(devicePixelRatio||1,2);canvas.width=width*ratio;canvas.height=height*ratio;context.scale(ratio,ratio);
 const positions=lineups.map((p,i)=>pitchPositions(p,shapes[i]));
 let lastEvent=null,eventTime=-Infinity,previous=0,finished=false,attackWeight=.5;
 const teamColors=[color(profile(names[0])),color(profile(names[1]))];
 const rgb=hex=>[1,3,5].map(i=>parseInt(hex.slice(i,i+2),16));
 if(Math.hypot(...rgb(teamColors[0]).map((v,i)=>v-rgb(teamColors[1])[i]))<110)teamColors[1]='#e8eef6';
 const numberColors=teamColors.map(hex=>rgb(hex).reduce((sum,v,i)=>sum+v*[.2126,.7152,.0722][i],0)>155?'#162d32':'#fff');
 const ball={x:480,y:240},pitch=document.createElement('canvas');pitch.width=canvas.width;pitch.height=canvas.height;
 const field=pitch.getContext('2d');field.scale(ratio,ratio);field.fillStyle='#102d29';field.fillRect(0,0,width,height);
 for(let i=0;i<12;i++){field.fillStyle=i%2?'#164339':'#19483d';field.fillRect(40+i*73,35,73,410);}
 field.strokeStyle='#ffffff62';field.lineWidth=1.6;field.strokeRect(40,35,880,410);field.beginPath();field.moveTo(480,35);field.lineTo(480,445);field.stroke();field.beginPath();field.arc(480,240,60,0,Math.PI*2);field.stroke();field.strokeRect(40,145,130,190);field.strokeRect(790,145,130,190);field.strokeRect(40,195,42,90);field.strokeRect(878,195,42,90);field.fillStyle='#ffffff44';field.fillRect(27,211,13,58);field.fillRect(920,211,13,58);
 function draw(t){
  if(finished)return;const delta=t-previous;previous=t;const phase=t/650,elapsed=t-eventTime;
  context.drawImage(pitch,0,0,width,height);
  const attack=lastEvent?.side??Math.floor(phase/3)%2;
  const targetX=lastEvent&&elapsed<650?(attack===0?922:38):140+(Math.sin(phase*.9)*.5+.5)*680;
  const targetY=lastEvent&&elapsed<650?240+Math.sin(phase)*20:130+(Math.cos(phase*1.7)*.5+.5)*220;
  const blend=motionBlend(delta);ball.x+=(targetX-ball.x)*blend;ball.y+=(targetY-ball.y)*blend;attackWeight+=((attack===0?1:0)-attackWeight)*motionBlend(delta,280);
  context.strokeStyle='#cdebc73b';context.setLineDash([5,8]);context.beginPath();context.moveTo(ball.x-(attack===0?110:-110),ball.y+50);context.lineTo(ball.x,ball.y);context.stroke();context.setLineDash([]);
  positions.forEach((rows,side)=>rows.forEach(({player:p,x,y},index)=>{
   const px=side===0?65+x*370:895-x*370,py=50+y*375;
   const weight=side===0?attackWeight:1-attackWeight;
   const sx=px+Math.sin(phase*.6+index)*8+(weight*23-8)*(side===0?1:-1),sy=py+Math.cos(phase*.4+index)*9;
   context.fillStyle='#00000030';context.beginPath();context.ellipse(sx,sy+14,14,5,0,0,Math.PI*2);context.fill();
   context.fillStyle=teamColors[side];context.strokeStyle='#ffffffc0';context.lineWidth=1.4;context.beginPath();context.arc(sx,sy,12,0,Math.PI*2);context.fill();context.stroke();
   context.fillStyle=numberColors[side];context.font='bold 10px Arial';context.textAlign='center';context.textBaseline='middle';context.fillText(String(p.number||index+1),sx,sy);
  }));
  context.fillStyle='#00000040';context.beginPath();context.ellipse(ball.x,ball.y+7,7,3,0,0,Math.PI*2);context.fill();context.fillStyle='#fff';context.beginPath();context.arc(ball.x,ball.y,5.5,0,Math.PI*2);context.fill();context.strokeStyle='#163831';context.lineWidth=1.5;context.stroke();
  if(lastEvent?.kind==='goal'&&elapsed<850){context.fillStyle=`rgba(225,191,112,${.13*(1-elapsed/850)})`;context.fillRect(0,0,width,height);const random=seededRandom(seed+lastEvent.minute);for(let i=0;i<35;i++){const angle=random()*Math.PI*2,speed=30+random()*220;context.fillStyle=i%2?'#f0c66a':'#faf4df';const x=480+Math.cos(angle)*speed*elapsed/650,y=240+Math.sin(angle)*speed*elapsed/650+elapsed*elapsed*.00015;context.fillRect(x,y,4,7);}}
 }
 draw(0);
 return {update(t){draw(t);},event(e,t){lastEvent=e;eventTime=t;},finish(){finished=true;},destroy(){finished=true;}};
}
function bindSimulation(m,l){
 stopReplay();
 function updateXI(){const xis=simulationXI(m,l);$('simLineups').innerHTML=`<div class="sim-lineup-list">${xis.map((ps,side)=>`<div><strong>${esc(short([m.home,m.away][side]))} · ${esc([state.homeShape,state.awayShape][side])}</strong><p>${ps.map(p=>esc(p.name)).join(' · ')||'Squad data unavailable'}</p></div>`).join('')}</div>`;}
 updateXI();
 for(const [i,side] of ['home','away'].entries()){
  const t=profile([m.home,m.away][i]),key=String(t.id||t.name),hint=$(side+'ShapeHint');
  function changed(manual){state[side+'Shape']=$(side+'Shape').value;if(manual)formationOverrides.set(key,state[side+'Shape']);else formationOverrides.delete(key);stopReplay();$('simulationOutput').innerHTML='';hint.querySelector('span').textContent=(manual?'Your selection · Usual '+usualFormation(t,data.as_of).shape:usualFormation(t,data.as_of).basis+' · '+formationHint(t));hint.querySelector('button').hidden=!manual;updateXI();}
  $(side+'Shape').onchange=()=>changed(true);
  hint.querySelector('button').onclick=()=>{$(side+'Shape').value=usualFormation(t,data.as_of).shape;changed(false);};
 }
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
   const clock=replayClock(duration);clock.tick(performance.now());
   let minute=0,index=0,home=0,away=0,finished=false,renderedMinute=-1;
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
    replayStage?.event(e,clock.elapsed);
   }
   function update(target){
    if(!$('replayScore')){finished=true;stopReplay();return;}minute=Math.min(90,Math.floor(target));
    while(index<story.events.length&&story.events[index].minute<=minute)addEvent(story.events[index++]);
    replayStage?.update(clock.elapsed);$('replayProgress').style.transform=`scaleX(${Math.min(1,target/90)})`;
    if(minute!==renderedMinute){renderedMinute=minute;$('replayClock').textContent=minute+'′';$('replayPhase').textContent=minute<45?'FIRST HALF':'SECOND HALF';$('liveMatchStats').innerHTML=liveStats();$('storyCount').textContent=index+' moments';}
    if(minute===90&&!finished){finished=true;cancelAnimationFrame(replayTimer);replayTimer=null;replayStage?.finish();$('replayPhase').textContent='FULL TIME';$('pitchCaption').textContent=home===away?'Honours even.':short(names[home>away?0:1])+' take the win.';$('pauseReplay').hidden=true;$('skipReplay').hidden=true;
     const contributions=new Map();for(const e of match.events)for(const [p,weight] of [[e.scorer,2],[e.assister,1]])if(p)contributions.set(p.id,{p,side:e.side,score:(contributions.get(p.id)?.score||0)+weight});const star=[...contributions.values()].sort((a,b)=>b.score-a.score)[0],team=star?profile(names[star.side]):null,player=team?.players.find(p=>p.id===star?.p.id);
     $('replaySummary').innerHTML=`<div class="full-time-card"><div><div class="eyebrow">ONE POSSIBLE OUTCOME</div><h3>${home===away?'The points are shared.':esc(short(names[home>away?0:1]))+' win.'}</h3><p>Final score <strong>${home}–${away}</strong> · ${match.events.length} ${match.events.length===1?'goal':'goals'} · ${story.shots[0]+story.shots[1]} simulated shots</p></div>${player?playerButton(team,player,`${avatar(player)}<span><small>SIMULATION SPOTLIGHT</small><strong>${esc(player.name)}</strong></span>`,'match-star'):''}</div><details class="ensemble"><summary>Explore 10,000 possible outcomes</summary>${probabilities({home_win:aggregate.outcomes[0]/10000,draw:aggregate.outcomes[1]/10000,away_win:aggregate.outcomes[2]/10000})}<p>Average goals: <strong>${aggregate.goals.map(x=>x.toFixed(2)).join(' – ')}</strong>. A sampled score is one outcome, not a prediction guarantee.</p><small>Replay seed ${seed}</small></details><p class="fine-print">Score sampled from the team model. Shot counts, possession, event timings and contributions are synthetic and are not outputs of a trained tactical or player model.</p>`;
     $('runSimulation').innerHTML='<span>↻</span> Play another match';
    }
   }
   $('liveMatchStats').innerHTML=liveStats();
   $('pauseReplay').onclick=()=>{clock.pause(!clock.paused,performance.now());$('pauseReplay').closest('.broadcast').classList.toggle('is-paused',clock.paused);$('pauseReplay').textContent=clock.paused?'Resume':'Pause';if(clock.paused){cancelAnimationFrame(replayTimer);replayTimer=null;}else replayTimer=requestAnimationFrame(tick);};
   $('skipReplay').onclick=()=>update(90);
   function tick(now){update(clock.tick(now)*90);if(!finished&&!clock.paused)replayTimer=requestAnimationFrame(tick);}
   if(duration===0)update(90);else replayTimer=requestAnimationFrame(tick);
   document.querySelector('.broadcast')?.scrollIntoView({behavior:reduced?'instant':'smooth',block:'nearest'});
  }catch(e){$('simulationOutput').textContent=e.message;}
 };
}
