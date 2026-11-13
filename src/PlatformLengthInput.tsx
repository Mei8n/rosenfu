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
export function PlatformLengthInput({id,label,value,onChange}:{id:string;label:string;value:string;onChange:(value:string)=>void}) {
 const [cars,setCars]=useState(()=>convertPlatformLength(value,true));
 const expected=useRef(value);
 useEffect(()=>{if(value!==expected.current){setCars(convertPlatformLength(value,true));expected.current=value;}},[value]);
 return <div><label htmlFor={id+'-cars'}>{label}</label><input id={id+'-cars'} type="number" required min="1" max="50" step="any" value={cars} onChange={event=>{setCars(event.target.value);const metres=convertPlatformLength(event.target.value,false);expected.current=metres;onChange(metres);}}/>
 <small className="field-hint">1両相当＝20m。小数可・整数への丸めなし。車両収容判定ではありません。</small></div>;
}
