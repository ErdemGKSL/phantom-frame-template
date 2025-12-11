import fs from 'fs';
import path from 'path';

const buildDir = 'build';
const PORT = '13472';

function replacePortInFiles(dir) {
	const files = fs.readdirSync(dir);

	files.forEach(file => {
		const filePath = path.join(dir, file);
		const stat = fs.statSync(filePath);

		if (stat.isDirectory()) {
			replacePortInFiles(filePath);
		} else if (file.endsWith('.js')) {
			let content = fs.readFileSync(filePath, 'utf-8');
			const modified = content.replace(/env\("PORT",\s*"3000"\)/g, `env("PORT", "${PORT}")`);

			if (modified !== content) {
				fs.writeFileSync(filePath, modified, 'utf-8');
			}
		}
	});
}

if (fs.existsSync(buildDir)) {
	replacePortInFiles(buildDir);
	console.log('Post-build script completed!');
} else {
	console.log(`Build directory not found: ${buildDir}`);
}
