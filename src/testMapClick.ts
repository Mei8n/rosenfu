import {act,fireEvent} from '@testing-library/react';
/** Let native placement verdicts settle before the next simulated gesture. */
export async function settledMapClick(...args:Parameters<typeof fireEvent.click>){
 await act(async()=>{fireEvent.click(...args);});
}
