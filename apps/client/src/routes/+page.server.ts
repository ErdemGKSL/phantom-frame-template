import type { PageServerLoad } from './$types';
import { apiFetch } from '$lib/api';

export const load: PageServerLoad = async () => {
    const data = await apiFetch<{ value: number }>('/api/counter');
    console.log({ data });
    return { counter: data.value };
};
