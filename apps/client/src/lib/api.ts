import { browser } from '$app/environment';
import { env } from '$env/dynamic/public';

function resolveUrl(path: string): string {
    if (browser) {
        return path;
    }
    const port = env.PUBLIC_RUST_SERVER_PORT ?? '3030';
    return `http://localhost:${port}${path}`;
}

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
    const res = await fetch(resolveUrl(path), init);
    if (!res.ok) {
        throw new Error(`API request failed: ${res.status} ${res.statusText}`);
    }
    return res.json() as Promise<T>;
}
