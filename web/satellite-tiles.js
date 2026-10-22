/* Display-only global WMTS. The WGS84 grid matches the game's geographic
 * projection; no Mercator image is stretched over the local plane.
 * Only the pinned CC BY 4.0 2016 layer is enabled (see maps/eox-source.json).
 */
(function(root){
  'use strict';
  const LAYER='eox-s2cloudless-2016', LIMIT=256, CONCURRENT=6;
  const credit='EOxCloudless https://cloudless.eox.at by EOX IT Services GmbH (Contains modified Copernicus Sentinel data 2016 & 2017)';
  function layout(theater,toScreen,width,height){
    const k=60*Math.cos(theater.reference_lat*Math.PI/180);
    const origin=toScreen(0,0),east=toScreen(k,0),north=toScreen(0,60);
    const sx=east.x-origin.x,sy=north.y-origin.y;
    if(!(sx>0&&sy<0&&width>0&&height>0))return [];
    const west=Math.max(-180,theater.west-origin.x/sx),right=Math.min(180,theater.west+(width-origin.x)/sx);
    const south=Math.max(-90,theater.south+(height-origin.y)/sy),top=Math.min(90,theater.south-origin.y/sy);
    if(west>=right||south>=top)return [];
    let z=Math.max(0,Math.min(13,Math.ceil(Math.log2(180*Math.max(sx,-sy)/256))));
    const range=level=>{const span=180/2**level;return {span,c0:Math.floor((west+180)/span),c1:Math.ceil((right+180)/span)-1,r0:Math.floor((90-top)/span),r1:Math.ceil((90-south)/span)-1};};
    let r=range(z);
    while((r.c1-r.c0+1)*(r.r1-r.r0+1)>128&&z>0)r=range(--z);
    const tiles=[];
    for(let row=r.r0;row<=r.r1;row++)for(let col=r.c0;col<=r.c1;col++){
      const lon=-180+col*r.span,lat=90-row*r.span;
      const a=toScreen((lon-theater.west)*k,(lat-theater.south)*60),b=toScreen((lon+r.span-theater.west)*k,(lat-r.span-theater.south)*60);
      tiles.push({key:`${z}/${row}/${col}`,z,row,col,x:a.x,y:a.y,width:b.x-a.x,height:b.y-a.y});
    }
    return tiles;
  }
  const cache=new Map();let active=0,queue=[],wanted=new Set(),redraw=null,failed=0;
  function pump(){
    while(active<CONCURRENT&&queue.length){
      const key=queue.shift(),item=cache.get(key);
      if(!item||item.state!=='queued'||!wanted.has(key))continue;
      active++;item.state='loading';const image=item.image=new root.Image();
      let finished=false;const timer=setTimeout(()=>done(false),15000);
      const done=ok=>{if(finished)return;finished=true;clearTimeout(timer);active--;item.state=ok?'ready':'error';if(!ok){failed++;item.retry=Date.now()+60000;}pump();redraw?.();};
      image.onload=()=>done(true);image.onerror=()=>done(false);
      image.src=`https://tiles.maps.eox.at/wmts/1.0.0/s2cloudless/default/WGS84/${key}.jpg`;
    }
  }
  function draw(ctx,theater,toScreen,width,height,onChange){
    if(theater?.tile_layer!==LAYER)return null;
    redraw=onChange;const tiles=layout(theater,toScreen,width,height);wanted=new Set(tiles.map(t=>t.key));queue=[];
    let ready=0,errors=0;
    for(const t of tiles){
      let item=cache.get(t.key);
      if(!item){item={state:'queued'};cache.set(t.key,item);}
      else {cache.delete(t.key);cache.set(t.key,item);}
      if(item.state==='error'&&item.retry<=Date.now())item.state='queued';
      if(item.state==='queued')queue.push(t.key);
      if(item.state==='ready'){ctx.drawImage(item.image,t.x,t.y,t.width,t.height);ready++;}
      if(item.state==='error')errors++;
    }
    for(const [key,item] of cache){if(cache.size<=LIMIT)break;if(!wanted.has(key)&&item.state!=='loading')cache.delete(key);}
    pump();return {ready,total:tiles.length,errors,credit,zoom:tiles[0]?.z};
  }
  const api={layout,draw,credit,layer:LAYER,get status(){return {cached:cache.size,active,failed};}};
  root.HarpoonSatelliteTiles=api;if(typeof module!=='undefined')module.exports=api;
})(typeof window!=='undefined'?window:globalThis);
