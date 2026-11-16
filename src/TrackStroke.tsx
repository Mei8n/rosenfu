import type {SVGProps} from 'react';
/** A quiet map-colour casing separates a rail from roads and map labels.
 * Both strokes are presentation-only; hit targets remain owned by the editor. */
export function TrackStroke({className='',...props}:SVGProps<SVGPathElement>){
 return <g pointerEvents="none"><path d={props.d} className="rail-casing" aria-hidden="true"/><path {...props} className={className} pointerEvents="none"/></g>;
}
