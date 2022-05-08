import { defineConfig } from 'vite'

// import dotenv from 'dotenv';
// dotenv.config({ path: '.env.local' });



export default defineConfig({
	test: {
		// clearMocks: true,
		testTimeout: 25000,
		mockReset: true,
		maxThreads: 1,
		minThreads: 1
	},
})