import type {SectionInfrastructure} from './sectionInfrastructure';
/** User-approved relative multipliers. No base prices or economic effects are implemented yet. */
export const HEIGHT_COST_FACTORS:Record<SectionInfrastructure['level'],{construction:number;maintenance:number}>={
 elevated_3:{construction:2,maintenance:1.2},elevated_2:{construction:1.5,maintenance:1},elevated:{construction:1,maintenance:1},
 ground:{construction:.5,maintenance:.7},underground:{construction:1,maintenance:1.5},underground_2:{construction:1.7,maintenance:2.5},underground_3:{construction:2,maintenance:2.5}
};
