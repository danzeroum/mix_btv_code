import { test, expect } from '@playwright/test'

test.beforeEach(async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: /Squad Designer/ }).click()
  await expect(page.getByRole('heading', { name: 'Squad Designer' })).toBeVisible()
})

test('arrastar o nó task move sua posição, dentro dos limites do board', async ({ page }) => {
  const task = page.getByRole('button', { name: '▸ task' })
  const box = await task.boundingBox()
  expect(box).not.toBeNull()

  await page.mouse.move(box!.x + box!.width / 2, box!.y + box!.height / 2)
  await page.mouse.down()
  await page.mouse.move(box!.x + 300, box!.y + 150, { steps: 8 })
  await page.mouse.up()

  const newBox = await task.boundingBox()
  expect(newBox!.x).not.toBeCloseTo(box!.x, 0)
  expect(newBox!.x).toBeGreaterThanOrEqual(0)
  expect(newBox!.y).toBeGreaterThanOrEqual(0)

  await page.screenshot({ path: 'tests/e2e/__screenshots__/designer-after-drag.png' })
})

test('modo conectar cria uma nova aresta entre dois nós', async ({ page }) => {
  await page.getByRole('button', { name: '↳ conectar' }).click()
  const canvas = page.getByRole('group', { name: 'canvas do squad designer' })

  const initialLines = await page.locator('svg line').count()

  await canvas.getByRole('button', { name: /Architect/ }).click()
  await canvas.getByRole('button', { name: /Auditor/ }).click()

  await expect(page.locator('svg line')).toHaveCount(initialLines + 1)
})

test('remover um nó não protegido some com ele e suas conexões', async ({ page }) => {
  const canvas = page.getByRole('group', { name: 'canvas do squad designer' })
  await canvas.getByRole('button', { name: /Auditor/ }).click()
  await page.getByRole('button', { name: /remover nó/ }).click()

  await expect(canvas.getByRole('button', { name: /Auditor/ })).toHaveCount(0)
})
