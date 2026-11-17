// @vitest-environment jsdom
import {cleanup,render,screen} from '@testing-library/react';
import {afterEach,expect,test} from 'vitest';
import {CrossingEstimate} from './CrossingEstimate';
afterEach(cleanup);
test('shows crossing count and an explicitly uncharged reference cost',()=>{
 render(<CrossingEstimate value={{newCrossingCount:2,roadsUnverified:false}}/>);
 expect(screen.getByText('踏切 2か所')).toBeTruthy();expect(screen.getByText('追加額（参考）2億円')).toBeTruthy();expect(screen.getByText(/課金未実装/)).toBeTruthy();
});
test('partial road data does not imply complete zero crossings',()=>{
 render(<CrossingEstimate value={{newCrossingCount:0,roadsUnverified:true}}/>);
 expect(screen.getByText('踏切 0か所（確認済み分）')).toBeTruthy();expect(screen.getByText(/未確認部分/)).toBeTruthy();
});
test('pending or invalid data cannot present an old or fabricated count',()=>{
 const {rerender}=render(<CrossingEstimate pending value={{newCrossingCount:2,roadsUnverified:false}}/>);
 expect(screen.getByText('踏切：確認中')).toBeTruthy();expect(screen.queryByText(/2か所/)).toBeNull();
 rerender(<CrossingEstimate value={{newCrossingCount:-1,roadsUnverified:false}}/>);expect(screen.getByText('踏切：確認中')).toBeTruthy();
});

test.each([
 null,{}, {newCrossingCount:'2',roadsUnverified:false},
 {newCrossingCount:1.5,roadsUnverified:false},
 {newCrossingCount:Infinity,roadsUnverified:false},
 {newCrossingCount:2,roadsUnverified:'false'},
 {newCrossingCount:2},
])('malformed IPC summary remains pending: %j',value=>{
 render(<CrossingEstimate value={value}/>);
 expect(screen.getByText('踏切：確認中')).toBeTruthy();
 expect(screen.queryByText(/追加額/)).toBeNull();
});
test('a validation error suppresses even a previously valid crossing summary',()=>{
 render(<CrossingEstimate value={{newCrossingCount:2,roadsUnverified:false,validationError:'建設不可'}}/>);
 expect(screen.queryByText(/踏切|追加額/)).toBeNull();
});
