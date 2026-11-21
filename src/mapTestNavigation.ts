import {fireEvent} from '@testing-library/react';
import {DEFAULT_CAMERA,panCamera} from './map';
/** Pan through the real middle-button gesture, in map viewbox pixels. */
export function panMap(dx:number,dy=0){
 const map=document.querySelector<SVGSVGElement>('svg[role="img"]')!;
 const oldRect=map.getBoundingClientRect;
 map.getBoundingClientRect=()=>({x:0,y:0,left:0,top:0,right:1000,bottom:600,width:1000,height:600,toJSON:()=>({})});
 const pointer=(type:string,x:number,y:number)=>{const event=new Event(type,{bubbles:true,cancelable:true});Object.assign(event,{clientX:x,clientY:y,pointerId:99,button:1,isPrimary:true});fireEvent(map,event);};
 // Keep release inside the viewport, even for large camera movement.
 const steps=Math.max(1,Math.ceil(Math.max(Math.abs(dx)/400,Math.abs(dy)/200)));
 for(let n=0;n<steps;n++){pointer('pointerdown',500,300);pointer('pointermove',500-dx/steps,300-dy/steps);pointer('pointerup',500-dx/steps,300-dy/steps);}
 map.getBoundingClientRect=oldRect;
}
/** Infer the visible camera from the georeferenced sample grid, not removed UI. */
export function sampleCamera(){
 const transform=document.querySelector('[data-sample-scene]')!.getAttribute('patternTransform')!;
 const [x,y,scale]=transform.match(/-?[\d.]+(?:e[+-]?\d+)?/g)!.map(Number);
 const zoom=DEFAULT_CAMERA.zoom+Math.log2(scale);
 return panCamera({...DEFAULT_CAMERA,zoom},500-x-500*scale,300-y-300*scale);
}
