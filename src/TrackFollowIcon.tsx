export function TrackFollowIcon({kind}:{kind:'rail'|'road'|'parallel'}){
 return <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true"><g fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
 {kind==='road'?<><path d="M5 21 8 3M19 21 16 3"/><path strokeDasharray="3 3" d="M12 21V3"/></>:kind==='rail'?<><path d="M5 21V12L10 3M10 21V12L15 3M10 14l9-8"/><path d="M3 18h9M3 14h9M7 7l7 3"/></>:<><path d="M3 20C13 20 17 13 18 3M3 15C10 15 12 10 13 3"/><path strokeDasharray="2 3" d="M3 10C6 10 7 7 8 3"/></>}
 </g></svg>;
}
