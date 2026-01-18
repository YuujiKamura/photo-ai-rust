import { test, expect } from '@playwright/test';

test('photo-ai web smoke', async ({ page }) => {
  await page.goto('');

  const heading = page.locator('h1');
  await expect(heading).toBeVisible();
  await expect(heading).toContainText('Photo AI');

  const apiKeyInput = page.locator('#api-key');
  await expect(apiKeyInput).toBeVisible();

  const uploadArea = page.locator('.upload-area');
  await expect(uploadArea).toBeVisible();
  await expect(uploadArea).toHaveClass(/disabled/);

  await apiKeyInput.fill('dummy-key');
  await expect(uploadArea).not.toHaveClass(/disabled/);

  await expect(page.getByRole('button', { name: /AI/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /PDF/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /Excel/ })).toBeVisible();
});
