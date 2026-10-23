// Stock Theatre UI is contained inside the current Animation surface, so it
// cannot cover the native inspector or Sproket workstation.
const magicTheatre={frame:null,key:null,part:null,rotation:0,ready:false,syncing:false};
function theatreMount(container){
 const status=el('span',{id:'theatre-status',class:'muted'},'Loading Theatre…');
 const frame=el('iframe',{id:'theatre-timeline',title:'Theatre motion timeline',src:'theatre-frame.html'});
 magicTheatre.frame=frame;magicTheatre.key=null;magicTheatre.ready=false;magicTheatre.rotation=0;
 container.append(el('div',{class:'theatre-heading'},el('strong',{},'Theatre'),status),frame);
}
function theatreContext(){
 if(!magicTheatre.ready||!motion.meta)return;
 const part=motion.selected||motion.meta.layers[0]?.id;if(!part)return;
 const key=JSON.stringify([state.project,motion.character,part]);if(key===magicTheatre.key)return;
 magicTheatre.key=key;magicTheatre.part=part;magicTheatre.rotation=0;
 magicTheatre.frame.contentWindow.postMessage({magicTheatre:true,type:'context',project:state.project,character:motion.character,part},location.origin);
}
function theatreSeek(time){
 theatreContext();
 if(magicTheatre.ready&&!magicTheatre.syncing)magicTheatre.frame.contentWindow.postMessage({magicTheatre:true,type:'seek',time},location.origin);
}
window.addEventListener('message',event=>{
 if(event.source!==magicTheatre.frame?.contentWindow||!event.data?.magicTheatre)return;
 const message=event.data;
 if(message.type==='loaded'){magicTheatre.ready=true;theatreContext();}
 if(message.type==='ready')$('theatre-status').textContent=`${message.part} · local edits not saved`;
 if(message.type==='values'&&message.part===magicTheatre.part){magicTheatre.rotation=message.rotation;animationDraw();}
 if(message.type==='seek'&&motion.sample){animationPause();magicTheatre.syncing=true;motion.time=Math.max(0,Math.min(message.time,motion.sample.duration));animationDraw();magicTheatre.syncing=false;}
 if(message.type==='error')$('theatre-status').textContent=`Theatre failed: ${message.error}`;
});
function theatreTransform(ctx,layer){
 if(layer.id!==magicTheatre.part||!magicTheatre.rotation)return;
 const pivot=layer.pivot||[0,0];
 ctx.translate(pivot[0],pivot[1]);ctx.rotate(magicTheatre.rotation*Math.PI/180);ctx.translate(-pivot[0],-pivot[1]);
}
