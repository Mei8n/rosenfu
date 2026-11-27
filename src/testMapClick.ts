import {vi} from 'vitest';
import {act,fireEvent} from '@testing-library/react';
/** Let native placement verdicts settle before the next simulated gesture. */
export async function settledMapClick(...args:Parameters<typeof fireEvent.click>){
 await act(async()=>{fireEvent.click(...args);});
}

/** Cursor previews are published on animation frames; committing gestures remain synchronous. */
export async function settledPointerMove(...args:Parameters<typeof fireEvent.pointerMove>){
 await act(async()=>{fireEvent.pointerMove(...args);if(typeof requestAnimationFrame==='function'){if(vi.isFakeTimers())vi.advanceTimersToNextFrame();else await new Promise<void>(resolve=>requestAnimationFrame(()=>resolve()));}});
}
export async function settledPointerEnter(...args:Parameters<typeof fireEvent.pointerEnter>){
 await act(async()=>{fireEvent.pointerEnter(...args);if(typeof requestAnimationFrame==='function'){if(vi.isFakeTimers())vi.advanceTimersToNextFrame();else await new Promise<void>(resolve=>requestAnimationFrame(()=>resolve()));}});
}
