import { test, expect } from '@playwright/test';

test('real CLI findings, rule selection, example switch, syntax errors', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto('/');
  await expect(page.getByRole('heading', { name: '코드 입력' })).toBeVisible();
  await expect(page.getByRole('checkbox')).toHaveCount(3);
  await page.getByRole('button', { name: '코드 검사하기' }).click();
  await expect(page.getByText('2개 발견')).toBeVisible();
  await expect(page.getByText('useEffect에 setState를 넣어서는 안됩니다', { exact: true })).toBeVisible();
  await page.screenshot({ path: 'test-results/playground-desktop.png', fullPage: true });
  await page.getByRole('checkbox').nth(0).uncheck();
  await page.getByRole('button', { name: '코드 검사하기' }).click();
  await expect(page.getByText('1개 발견')).toBeVisible();
  await page.getByRole('combobox').selectOption('clean');
  await page.getByRole('button', { name: '코드 검사하기' }).click();
  await expect(page.getByText('선택한 규칙을 모두 통과했습니다')).toBeVisible();
  await page.getByRole('textbox', { name: 'TypeScript 코드' }).fill('const broken: = 1;');
  await page.getByRole('button', { name: '코드 검사하기' }).click();
  await expect(page.getByText('문법 오류', { exact: true })).toBeVisible();
  await page.getByRole('checkbox').nth(1).uncheck();
  await page.getByRole('checkbox').nth(2).uncheck();
  await expect(page.getByRole('button', { name: '코드 검사하기' })).toBeDisabled();
  expect(errors).toEqual([]);
});

test('mobile layout and server validation', async ({ page, request }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: '검사 규칙' })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);
  await page.getByRole('combobox').selectOption('alert');
  await page.getByRole('button', { name: '코드 검사하기' }).click();
  await expect(page.getByText('2개 발견')).toBeVisible();
  await page.screenshot({ path: 'test-results/playground-mobile.png', fullPage: true });
  for (const data of [{ source: 'alert(1)', rules: [] }, { source: 'alert(1)', rules: ['../../private'] }]) {
    const response = await request.post('/api/check', { data });
    expect(response.status()).toBe(400);
  }
});
