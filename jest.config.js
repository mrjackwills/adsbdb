module.exports = {
	preset: 'ts-jest',
	roots: [
		'<rootDir>/src'
	],
	testMatch: [
		'**/__tests__/**/*.+(ts|tsx|js)',
		'**/?(*.)+(spec|test).+(ts|tsx|js)'
	],
	transform: {
		'^.+\\.(ts|tsx)$': 'ts-jest'
	},
	testPathIgnorePatterns: [ 'testHelper.ts', 'jestSettings.ts' ],
	coveragePathIgnorePatterns: [ './src/__tests__/testHelper.ts', 'jestSettings.ts', './dist/*' ],
	setupFilesAfterEnv: [ './src/__tests__/jestSettings.ts' ],

};