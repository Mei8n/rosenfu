// @vitest-environment jsdom
import {afterEach,expect,test} from 'vitest';
import {cleanup,fireEvent,render,screen} from '@testing-library/react';
import {DistanceFareQuote,quoteDistanceFare} from './DistanceFareQuote';

afterEach(cleanup);
const fictionalBands=[{maxMetres:'3000',fareYen:'190'},{maxMetres:'7000',fareYen:'240'}];

test('quotes one whole journey from the supplied shortest distance, with inclusive bounds',()=>{
 expect(quoteDistanceFare(true,'3000',fictionalBands)).toEqual({status:'quoted',fareYen:190,maxMetres:3000});
 expect(quoteDistanceFare(true,'3001',fictionalBands)).toEqual({status:'quoted',fareYen:240,maxMetres:7000});
 // Direct or split by a transfer within this area, the single confirmed shortest OD is 4 km.
 expect(quoteDistanceFare(true,'4000',fictionalBands)).toEqual({status:'quoted',fareYen:240,maxMetres:7000});
 expect(2*190).not.toBe(240);
 expect(quoteDistanceFare(true,'7000',fictionalBands)).toEqual({status:'quoted',fareYen:240,maxMetres:7000});
 expect(quoteDistanceFare(true,'7001',fictionalBands)).toEqual({status:'outside'});
});

test('never guesses missing evidence, malformed bounds, or an outside-table fare',()=>{
 expect(quoteDistanceFare(false,'4000',fictionalBands)).toEqual({status:'unverified'});
 for(const bad of ['','0','-1','1e3','3.5','10000001'])expect(quoteDistanceFare(true,bad,fictionalBands)).toEqual({status:'distance'});
 for(const bad of [[],[...fictionalBands].reverse(),[fictionalBands[0],fictionalBands[0]],
  [...fictionalBands,{maxMetres:'9000',fareYen:'-5'}],Array.from({length:13},()=>fictionalBands[0])]){
  expect(quoteDistanceFare(true,'2000',bad)).toEqual({status:'table'});
 }
 expect(quoteDistanceFare(true,'1',[{maxMetres:'3000',fareYen:'0'}])).toEqual({status:'quoted',fareYen:0,maxMetres:3000});
});

test('requires an explicit same-area confirmation and clears old quotes on editing',()=>{
 render(<DistanceFareQuote/>);
 fireEvent.click(screen.getByText('距離帯運賃の手入力試算'));
 const enter=(label:string,value:string)=>fireEvent.change(screen.getByLabelText(label),{target:{value}});
 const submit=()=>fireEvent.click(screen.getByRole('button',{name:'距離帯運賃を見積もる'}));
 enter('確認済みの最短営業キロ（m）','4000');
 enter('区分 1 の上限（m）','3000');
 enter('区分 1 の運賃（円）','190');
 fireEvent.click(screen.getByRole('button',{name:'区分を追加'}));
 enter('区分 2 の上限（m）','7000');
 enter('区分 2 の運賃（円）','240');
 submit();
 expect(screen.getByRole('alert').textContent).toContain('同じ運賃圏');
 fireEvent.click(screen.getByLabelText('出発地から到着地まで同じ運賃圏と確認済み'));
 submit();
 expect(screen.getByRole('status').textContent).toContain('240円（7,000 mまで）');
 enter('確認済みの最短営業キロ（m）','3000');
 expect(screen.queryByRole('status')).toBeNull();
 submit();
 expect(screen.getByRole('status').textContent).toContain('190円（3,000 mまで）');
 fireEvent.click(screen.getByRole('button',{name:'最後の区分を削除'}));
 expect(screen.queryByRole('status')).toBeNull();
 enter('確認済みの最短営業キロ（m）','4000');
 submit();
 expect(screen.getByRole('alert').textContent).toContain('運賃は推定しません');
});
