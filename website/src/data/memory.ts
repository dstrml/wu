import type { Messages } from '../i18n/en';

export interface MemoryScenario {
  id: keyof Messages['memoryChart']['scenarios'];
  results: { editor: string; mib: number }[];
}

export const memoryScenarios: MemoryScenario[] = [
  {
    id: 'idle',
    results: [
      { editor: 'Wu', mib: 382.5 },
      { editor: 'Zed', mib: 420.4 },
      { editor: 'VS Code', mib: 743.5 }
    ]
  },
  {
    id: 'search',
    results: [
      { editor: 'Wu', mib: 582.9 },
      { editor: 'Zed', mib: 1395.3 },
      { editor: 'VS Code', mib: 823.6 }
    ]
  }
];
