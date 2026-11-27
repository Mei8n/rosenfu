// @vitest-environment jsdom
import {useState} from 'react';
import {afterEach,expect,test} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {StandardRailFrequencyFields,parseFrequencyDraft,type FrequencyDraft} from './StandardRailFrequencyFields';

afterEach(cleanup);

test('gives all five standard time bands without moving peak increases into advanced settings',()=>{
 let observed:FrequencyDraft|undefined;
 function Harness(){
  const [draft,setDraft]=useState<FrequencyDraft>({baseMinutes:'10',overrides:{}});
  return <StandardRailFrequencyFields draft={draft} onChange={value=>{observed=value;setDraft(value);}}/>;
 }
 render(<Harness/>);
 for(const name of ['早朝','朝ラッシュ','昼','夕ラッシュ','深夜']){
  const input=screen.getByRole('spinbutton',{name:`${name}の間隔（分）`}) as HTMLInputElement;
  expect(input.value).toBe('');
 }
 fireEvent.change(screen.getByLabelText('朝ラッシュの間隔（分）'),{target:{value:'5'}});
 fireEvent.change(screen.getByLabelText('夕ラッシュの間隔（分）'),{target:{value:'6'}});
 expect(parseFrequencyDraft(observed!)).toEqual({baseMinutes:10,overrides:{morningRush:5,eveningRush:6}});
 fireEvent.change(screen.getByLabelText('朝ラッシュの間隔（分）'),{target:{value:''}});
 expect(parseFrequencyDraft(observed!)).toEqual({baseMinutes:10,overrides:{eveningRush:6}});
});

test('keeps absent overrides apart from invalid values; nothing is rounded or silently applied',()=>{
 expect(parseFrequencyDraft({baseMinutes:'10',overrides:{lateNight:''}})).toEqual({baseMinutes:10,overrides:{}});
 for(const invalid of ['', '01','0','1.5','-3','241','NaN']){
  expect(parseFrequencyDraft({baseMinutes:invalid,overrides:{}})).toBeNull();
  if(invalid)expect(parseFrequencyDraft({baseMinutes:'10',overrides:{lateNight:invalid}})).toBeNull();
 }
});
