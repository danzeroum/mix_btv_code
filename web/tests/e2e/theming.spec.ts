import { test, expect } from '@playwright/test'

async function cssVar(page: import('@playwright/test').Page, name: string) {
  return page.evaluate((n) => {
    const el = document.getElementById('forge-root')
    return el ? getComputedStyle(el).getPropertyValue(n).trim() : ''
  }, name)
}

test('trocar de tema aplica os valores exatos de --rust do README §8.3', async ({ page }) => {
  await page.goto('/')

  expect(await cssVar(page, '--rust')).toBe('#f2683c')

  await page.getByRole('button', { name: 'Ultramarino', exact: true }).click()
  expect(await cssVar(page, '--rust')).toBe('#3f6fd6')

  await page.getByRole('button', { name: 'Mármore', exact: true }).click()
  expect(await cssVar(page, '--rust')).toBe('#b0532f')
})

test('accent sobrepõe --rust independente do tema, e persiste após reload', async ({ page }) => {
  await page.goto('/')

  await page.getByTitle('ouro de Ticiano').click()
  expect(await cssVar(page, '--rust')).toBe('#c8972f')

  await page.reload()
  expect(await cssVar(page, '--rust')).toBe('#c8972f')
})

test('tema persiste em localStorage após reload', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Afresco' }).click()
  expect(await cssVar(page, '--bg')).toBe('#efe4cf')

  await page.reload()
  expect(await cssVar(page, '--bg')).toBe('#efe4cf')
})
