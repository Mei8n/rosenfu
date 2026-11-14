import { useEffect, useRef, useState } from "react";
/** Decimal conversion avoids rounding stored metre values to whole car counts. */
export function convertPlatformLength(value: string, toCars: boolean): string {
  if (/e/i.test(value)) { const number=Number(value);if(!Number.isFinite(number) || Math.abs(number)>100000 || (number!==0 && Math.abs(number)<.000001)) return "";value=String(number); }
  if (!/^-?\d+(?:\.\d*)?$/.test(value)) return "";
  const negative=value.startsWith("-");const [whole,fraction=""]=value.replace(/^-/,'').split('.');
  const digits=(BigInt(whole+fraction)*(toCars?5n:20n)).toString();
  const scale=fraction.length+(toCars?2:0), padded=digits.padStart(scale+1,'0');
  const decimal=scale?padded.slice(0,-scale)+'.'+padded.slice(-scale):padded;
  return (negative && BigInt(digits)!==0n?'-':'')+decimal.replace(/(\.\d*?)0+$/,'$1').replace(/\.$/,'');
}
export function PlatformLengthInput({id,label,value,onChange,compact=false}:{id:string;label:string;value:string;onChange:(value:string)=>void;compact?:boolean}) {
 const [cars,setCars]=useState(()=>convertPlatformLength(value,true));
 const expected=useRef(value);
 useEffect(()=>{if(value!==expected.current){setCars(convertPlatformLength(value,true));expected.current=value;}},[value]);
 return <div className={compact?"station-length-control":undefined}><label htmlFor={id+'-cars'}>{label}</label>{compact&&<button type="button" aria-label="駅を1両短くする" disabled={Number(cars)<=2} onClick={()=>onChange(convertPlatformLength(String(Math.max(2,Math.min(20,Number(cars)-1))),false))}>−</button>}<input id={id+'-cars'} type="number" required min="2" max="20" step="any" value={cars} onChange={event=>{setCars(event.target.value);const metres=convertPlatformLength(event.target.value,false);expected.current=metres;onChange(metres);}}/>
 <span className={compact?undefined:"field-hint"}>両</span>{compact&&<button type="button" aria-label="駅を1両長くする" disabled={Number(cars)>=20} onClick={()=>onChange(convertPlatformLength(String(Math.max(2,Math.min(20,Number(cars)+1))),false))}>＋</button>}</div>;
}
